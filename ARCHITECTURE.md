# BASpark 架构重构方案 — evdev 输入接管

## 动机

当前 Rust 版 BASpark 依赖 Wayland `wlr-layer-shell` 协议的全屏 Overlay 表面来同时处理两件事：
1. **输入捕获** — 通过 `wl_pointer` 获取全局鼠标事件
2. **渲染输出** — 在透明 Overlay 上绘制粒子特效

这种架构在 niri 等非 wlroots 的 Smithay 系 compositor 上遇到多个兼容性问题：
- `wl_pointer::enter` 在表面创建于指针下方时不触发（niri Issue #1194）
- virtual_pointer 支持较晚添加（PR #630），坐标转换曾出现 bug
- 全屏 Overlay 的 input region 切换逻辑在不同 compositor 上行为不一致（Issue #892）
- 100ms 定时穿透窗口是脆弱的时序 hack，快速点击时易出现竞态

## 核心思路：输入与渲染解耦

```
┌──────────────────────────────────────────────────────┐
│                    BASpark Daemon                     │
├──────────────────────────────────────────────────────┤
│                                                      │
│  ┌─────────────────────┐  ┌──────────────────────┐  │
│  │   evdev 输入层       │  │  Wayland 渲染输出层  │  │
│  │  (input/evdev.rs)   │  │  (output/wayland.rs) │  │
│  │                      │  │                      │  │
│  │  从 /dev/input/event* │  │  纯 Overlay 渲染     │  │
│  │  被动读取事件         │  │  无输入处理          │  │
│  │  不 grab 设备        │  │  无 virtual_pointer  │  │
│  │                      │  │  无穿透通道          │  │
│  └──────────┬───────────┘  └──────────────────────┘  │
│             │                                         │
│             ▼                                         │
│  ┌─────────────────────┐                             │
│  │  CursorTracker      │                             │
│  │  - REL 增量累计      │                             │
│  │  - BTN 点击检测      │                             │
│  │  - 输出 (x,y,btn)   │                             │
│  └──────────┬──────────┘                             │
│             │                                         │
│             ▼                                         │
│  ┌─────────────────────┐                             │
│  │  ParticleEngine     │                             │
│  │  (renderer/mod.rs)  │                             │
│  │  不变               │                             │
│  └─────────────────────┘                             │
└──────────────────────────────────────────────────────┘
```

### 关键变化

| 方面 | 之前 | 之后 |
|------|------|------|
| **输入源** | `wl_pointer` (Wayland) | `evdev` (内核直读) |
| **位置获取** | 从 `wl_pointer::enter/motion` 的 absolute surface 坐标 | 从 `REL_X/REL_Y` 增量累计 |
| **点击检测** | `wl_pointer::button` 事件 | `BTN_LEFT/RIGHT/MIDDLE` evdev 事件 |
| **光标追踪** | pointer_entered flag + surface_x/y | CursorTracker 结构体 |
| **点击转发** | `virtual_pointer` + 100ms 穿透 hack | **不需要**（不消费事件） |
| **input region** | 频繁切换（全屏↔空） | 始终为空（仅渲染，不参与输入） |
| **Wayland 依赖** | wlr-layer-shell + virtual-pointer + wl_seat/pointer | 仅 wlr-layer-shell（渲染） |
| **compositor 兼容性** | 每个 compositor 需调试 | 内核级，全部兼容 |

## 模块结构

```
src/
├── main.rs               CLI 入口（不变，去掉 Reload 命令？保留）
├── app.rs                主循环（evdev poll + 渲染帧）
├── config.rs             配置管理（新增 input_sensitivity, cursor_start_*）
│
├── input/                新增：输入层
│   ├── mod.rs            InputManager 统一接口
│   ├── evdev.rs          evdev 设备发现 + 事件读取 + 热插拔
│   └── cursor.rs         CursorTracker REL 累计 + 点击检测
│
├── output/               重构：渲染输出层
│   ├── mod.rs            OutputManager 统一接口
│   └── wayland.rs        仅保留 Wayland overlay 渲染（从 overlay.rs 剥离输入代码）
│
├── renderer/             不变：粒子渲染引擎
│   ├── mod.rs            ParticleEngine
│   ├── trail.rs          鼠标拖尾
│   ├── wave.rs           点击波纹
│   ├── spark.rs          火花粒子
│   ├── pool.rs           对象池
│   └── dirty_rect.rs     脏矩形优化
│
└── autostart.rs          不变：XDG 自启动管理
```

## evdev 事件流

### 设备发现

```
/dev/input/
├── event0    键盘
├── event1    鼠标  ← 需要匹配: EV_REL + EV_KEY
├── event2    触控板 ← 需要匹配: EV_REL + EV_KEY
└── event3    游戏手柄
```

筛选条件：
- `supported_events().contains(EventType::RELATIVE)` — 支持相对运动
- `supported_events().contains(EventType::KEY)` — 支持按键

### 事件格式

```
// 移动（连续多个事件后跟 SYN_REPORT）
Event { type: EV_REL, code: REL_X, value: 3 }
Event { type: EV_REL, code: REL_Y, value: -1 }
Event { type: EV_SYN, code: SYN_REPORT, value: 0 }

// 点击
Event { type: EV_KEY, code: BTN_LEFT, value: 1 }    // 按下
Event { type: EV_SYN, code: SYN_REPORT, value: 0 }
Event { type: EV_KEY, code: BTN_LEFT, value: 0 }    // 释放
Event { type: EV_SYN, code: SYN_REPORT, value: 0 }
```

### 位置累计

```rust
struct CursorTracker {
    x: f64,       // 当前 X（像素坐标）
    y: f64,       // 当前 Y
    sensitivity: f64, // 灵敏度缩放（evdev 原始单位 → 像素）
}
```

鼠标的 `REL_X/REL_Y` 单位是"设备单位"（mickey），需要乘以灵敏度因子转为像素。
默认灵敏度 1.0 约等于 1000 DPI 鼠标在 1920x1080 屏幕上的表现。

### 校准策略

由于 evdev 不提供光标的绝对初始位置，采用**逐步校准**策略：

1. **启动时**：使用 config 中的 `cursor_start_x/y` 作为初始值（默认屏幕中心 960,540）
2. **运行时**：增量跟随完全正确（鼠标移动 100px，累计器也移动 100px）
3. **偏差容忍**：粒子特效对位置绝对精度不敏感，偏差几十像素无感知
4. **可选增强**：自动检测屏幕分辨率后自动设置初始位置到中心

## Wayland 渲染输出

### OutputSurface 的简化

剥离后的 `output/wayland.rs` 只做三件事：

1. **初始化**：连接 Wayland、创建 Overlay 表面、设置全屏 anchor + 空 input region
2. **渲染**：接收 `ParticleEngine` 输出的像素数据 → 写入 SHM buffer → commit
3. **清理**：断开连接

移除：
- `wl_seat` 绑定
- `wl_pointer` 绑定 + 事件处理
- `zwlr_virtual_pointer_manager_v1` 绑定
- `zwlr_virtual_pointer_v1` 绑定
- `empty_region` / `passthrough_until` / `forward_skip_buttons`
- `pointer_entered` / `cursor_surface_x/y` / `left/right/middle_down`
- `pending_input` / `take_input_events()` / `is_button_down()`
- `enable_passthrough()` / `check_passthrough()`

## 权限要求

用户需要加入 `input` 组以读取 `/dev/input/event*`：

```bash
sudo usermod -a -G input $USER
# 重新登录生效
```

设备节点权限（大多数发行版默认）：

```bash
crw-rw---- 1 root input 13, 64 ... /dev/input/event0
```

## 热插拔

使用 `udev` 监控子系统事件：

```rust
let mut monitor = udev::MonitorBuilder::new(&udev::Context::new()?)?
    .match_subsystem("input")?
    .listen()?;

for event in monitor.iter() {
    match event.event_type() {
        udev::EventType::Add => { /* 添加设备 */ }
        udev::EventType::Remove => { /* 移除设备 */ }
        _ => {}
    }
}
```

## 迁移步骤

1. 更新 `Cargo.toml` — 添加 `evdev = "0.13"`，保留现有 Wayland 渲染依赖
2. 创建 `src/input/` 模块（mod.rs, evdev.rs, cursor.rs）
3. 重构 `overlay.rs` → `output/wayland.rs`（剥离输入代码）
4. 重构 `app.rs`（evdev 事件循环 + 渲染循环）
5. 更新 `main.rs`（适配新模块路径）
6. 更新 `config.rs`（添加 `input_sensitivity`, `cursor_start_x/y`）
7. 编译测试
8. 清理：删除 `input.rs`（旧输入事件类型），整合到新模块

<div align="center">

![BASpark](https://socialify.git.ci/TurkeyC/BASpark-Wayland/image?description=1&descriptionEditable=Blue%20Archive%20Style%20Particle%20Effect%20for%20Linux/Wayland&font=Inter&forks=1&issues=1&logo=https%3A%2F%2Fraw.githubusercontent.com%2FDoomVoss%2FBASpark%2Fmain%2Fassets%2Flogo.png&name=1&pattern=Diagonal%20Stripes&pulls=1&stargazers=1&theme=Auto)

[![GitHub stars](https://img.shields.io/github/stars/TurkeyC/BASpark-Wayland?style=social)](https://github.com/TurkeyC/BASpark-Wayland/stargazers)
[![GitHub license](https://img.shields.io/github/license/TurkeyC/BASpark-Wayland)](https://github.com/TurkeyC/BASpark-Wayland/blob/main/LICENSE)
[![GitHub issues](https://img.shields.io/github/issues/TurkeyC/BASpark-Wayland)](https://github.com/TurkeyC/BASpark-Wayland/issues)

[快速下载](https://github.com/TurkeyC/BASpark-Wayland/releases/latest) | [反馈 Bug](https://github.com/TurkeyC/BASpark-Wayland/issues/new) | [功能建议](https://github.com/TurkeyC/BASpark-Wayland/issues/new)

</div>

---

# BASpark-Wayland

> **Linux/Wayland 原生桌面粒子特效工具：** 使用 Rust + tiny-skia 深度复刻《蔚蓝档案》UI 风格动效。

**BASpark-Wayland** 是一款轻量级的 Linux 桌面粒子特效工具，精确再现了《蔚蓝档案》(Blue Archive) 中的点击波纹、拖尾和星形火花动画。它通过 `wlr-layer-shell` 在 Wayland 上创建透明覆盖层，并使用纯 CPU 2D 矢量引擎渲染粒子。

本项目是 [DoomVoss](https://github.com/DoomVoss/BASpark) 原版 **BASpark**（Windows WPF + WebView2 版本）的 **Linux/Wayland 原生移植**。

---

## 功能特性

* **真实视觉还原** — 三种粒子子系统（点击波纹、旋转火花、鼠标拖尾）忠实还原 Blue Archive 的视觉效果。
* **Wayland 原生** — 使用 `zwlr-layer-shell-v1` 创建透明覆盖层，`wl_pointer` 捕获输入，`zwlr_virtual_pointer_v1` 实现点击穿透转发。
* **纯 CPU 渲染** — 基于 `tiny-skia` 纯 Rust 2D 矢量图形库，无需 GPU 或 WebView。
* **空闲时零开销** — 无活跃粒子时渲染器完全休眠。
* **完全可配置** — 粒子颜色、缩放、透明度、速度、拖尾行为、刷新率等均可通过 TOML 配置文件调整。
* **CLI 驱动** — 守护进程式生命周期管理：`start`、`stop`、`status`、`config`、`reload`。

---

## 快速开始

### 系统要求

* **操作系统:** 支持 `wlr-layer-shell` 的 Wayland 合成器（如 Sway、Hyprland、River、Wayfire）
* **合成器功能:** `wlr-layer-shell-unstable-v1`、`zwlr-virtual-pointer-v1`

### 安装

#### 从源码编译

```bash
git clone https://github.com/TurkeyC/BASpark-Wayland.git
cd BASpark-Wayland
cargo build --release
sudo cp target/release/baspark /usr/local/bin/
```

#### 下载预编译二进制

从 [Releases 页面](https://github.com/TurkeyC/BASpark-Wayland/releases/latest) 下载最新二进制，然后：

```bash
chmod +x baspark
sudo mv baspark /usr/local/bin/
```

### 使用方法

```bash
# 启动守护进程
baspark start

# 停止
baspark stop

# 检查运行状态
baspark status

# 查看当前配置
baspark config --show

# 修改粒子颜色
baspark config --color "255,100,100"

# 修改缩放
baspark config --scale 2.0

# 热重载配置（无需重启）
baspark reload
```

配置文件位于 `~/.config/BASpark/config.toml`。

---

## 编译指南

```bash
# Debug 编译
cargo build

# Release 编译
cargo build --release

# 直接运行
cargo run -- start
```

### 依赖项

* Rust 2021 edition（1.70+）
* Wayland 开发库（`libwayland-dev` 或等效包）
* 无运行时依赖 — 二进制完全自包含。

---

## 配置参考

| 配置项 | 默认值 | 说明 |
|--------|--------|------|
| `particle_color` | `"45,175,255"` | RGB 颜色字符串 `"R,G,B"` |
| `is_effect_enabled` | `true` | 总开关 |
| `effect_scale` | `1.5` | 粒子大小倍数 (0.5–3.0) |
| `effect_opacity` | `1.0` | 全局透明度 (0.1–1.0) |
| `use_linked_speed` | `true` | 拖尾和点击速度联动 |
| `effect_speed` | `1.0` | 联动时速度 (0.2–3.0) |
| `trail_speed` | `1.0` | 拖尾动画速度 (0.2–3.0) |
| `click_speed` | `1.0` | 点击动画速度 (0.2–3.0) |
| `trail_refresh_hz` | `40` | 渲染帧率 (10–240 Hz) |
| `enable_always_trail` | `false` | 不按鼠标时也显示拖尾 |
| `click_trigger` | `"left"` | 触发特效的鼠标键 (`left`/`right`/`both`) |
| `filter_mode` | `"disabled"` | 进程过滤模式 (`disabled`/`blacklist`/`whitelist`) |
| `filter_processes` | `[]` | 过滤的进程名列表 |
| `hide_in_fullscreen` | `true` | 全屏应用自动隐藏 |
| `show_on_desktop` | `true` | 桌面显示特效 |
| `autostart` | `false` | 随会话自启动 |

---

## 项目架构

```
src/
├── main.rs          # CLI 入口 (clap)
├── app.rs           # 主循环、信号处理、帧率管理
├── config.rs        # TOML 配置读写
├── input.rs         # 输入事件类型定义
├── overlay.rs       # Wayland wlr-layer-shell 覆盖层 + 输入捕获
├── autostart.rs     # XDG 自启动管理
└── renderer/
    ├── mod.rs       # ParticleEngine — 粒子引擎主控
    ├── trail.rs     # 鼠标拖尾渲染（渐变线段）
    ├── wave.rs      # 点击波纹/环状渲染
    ├── spark.rs     # 星形火花渲染（旋转三角形）
    ├── pool.rs      # 泛型对象池
    └── dirty_rect.rs# 脏矩形追踪（局部更新优化）
```

## 贡献者

<a href="https://github.com/TurkeyC/BASpark-Wayland/graphs/contributors">
  <img src="https://contrib.rocks/image?repo=TurkeyC/BASpark-Wayland" />
</a>

### 参与贡献

欢迎提交 Pull Request！请确保代码风格与现有保持一致，并通过 `cargo build` 编译。

---

## 免责声明

* 本软件为同人爱好非商业项目，严禁倒卖。
* 视觉风格灵感来源于 Nexon / Yostar《Blue Archive》，版权归原作者所有。
* 软件按"原样"提供，作者不对使用产生的任何损失承担责任。

---

## 开源许可

MIT License — 详见 [LICENSE](./LICENSE) 文件。

原版 BASpark（Windows 版）作者：[DoomVoss](https://github.com/DoomVoss/BASpark)

<div align="center">
  Made with ❤️
</div>

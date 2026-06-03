use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    // 外观设置
    #[serde(default = "default_color")]
    pub particle_color: String,
    #[serde(default = "default_true")]
    pub is_effect_enabled: bool,
    #[serde(default = "default_scale")]
    pub effect_scale: f64,
    #[serde(default = "default_opacity")]
    pub effect_opacity: f64,

    // 速度设置
    #[serde(default = "default_true")]
    pub use_linked_speed: bool,
    #[serde(default = "default_speed")]
    pub effect_speed: f64,
    #[serde(default = "default_speed")]
    pub trail_speed: f64,
    #[serde(default = "default_speed")]
    pub click_speed: f64,

    // 刷新率
    #[serde(default = "default_refresh_rate")]
    pub trail_refresh_hz: u32,

    // 行为设置
    #[serde(default = "default_false")]
    pub enable_always_trail: bool,
    #[serde(default = "default_true")]
    pub show_on_desktop: bool,
    #[serde(default = "default_true")]
    pub hide_in_fullscreen: bool,
    #[serde(default = "default_false")]
    pub enable_environment_filter: bool,

    // 点击触发
    #[serde(default = "default_trigger")]
    pub click_trigger: ClickTrigger,
    #[serde(default = "default_false")]
    pub enable_middle_click: bool,

    // 多屏
    #[serde(default)]
    pub enabled_monitors: Vec<String>,

    // 语言
    #[serde(default = "default_language")]
    pub language: String,

    // 输入灵敏度 (Linux 特有，evdev 原始单位到像素的缩放)
    #[serde(default = "default_sensitivity")]
    pub input_sensitivity: f64,

    // 光标起始位置 (Linux 特有，启动时的初始光标像素坐标)
    #[serde(default = "default_cursor_start")]
    pub cursor_start_x: f64,
    #[serde(default = "default_cursor_start")]
    pub cursor_start_y: f64,

    // 进程过滤
    #[serde(default)]
    pub filter_mode: FilterMode,
    #[serde(default)]
    pub filter_processes: Vec<String>,

    // 自动启动
    #[serde(default = "default_false")]
    pub autostart: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ClickTrigger {
    Left,
    Right,
    Both,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum FilterMode {
    Disabled,
    Blacklist,
    Whitelist,
}

impl Default for ClickTrigger {
    fn default() -> Self {
        ClickTrigger::Left
    }
}

impl Default for FilterMode {
    fn default() -> Self {
        FilterMode::Disabled
    }
}

fn default_color() -> String {
    "45,175,255".to_string()
}
fn default_true() -> bool {
    true
}
fn default_false() -> bool {
    false
}
fn default_scale() -> f64 {
    1.5
}
fn default_opacity() -> f64 {
    1.0
}
fn default_speed() -> f64 {
    1.0
}
fn default_refresh_rate() -> u32 {
    40
}
fn default_trigger() -> ClickTrigger {
    ClickTrigger::Left
}
fn default_language() -> String {
    String::new()
}
fn default_sensitivity() -> f64 {
    0.5
}
fn default_cursor_start() -> f64 {
    960.0
}

impl Default for Config {
    fn default() -> Self {
        Self {
            particle_color: default_color(),
            is_effect_enabled: default_true(),
            effect_scale: default_scale(),
            effect_opacity: default_opacity(),
            use_linked_speed: default_true(),
            effect_speed: default_speed(),
            trail_speed: default_speed(),
            click_speed: default_speed(),
            trail_refresh_hz: default_refresh_rate(),
            enable_always_trail: default_false(),
            show_on_desktop: default_true(),
            hide_in_fullscreen: default_true(),
            enable_environment_filter: default_false(),
            click_trigger: default_trigger(),
            enable_middle_click: default_false(),
            enabled_monitors: vec![],
            language: default_language(),
            input_sensitivity: default_sensitivity(),
            cursor_start_x: default_cursor_start(),
            cursor_start_y: default_cursor_start(),
            filter_mode: FilterMode::Disabled,
            filter_processes: vec![],
            autostart: default_false(),
        }
    }
}

impl Config {
    pub fn config_dir() -> Option<PathBuf> {
        directories::ProjectDirs::from("", "", "BASpark")
            .map(|d| d.config_dir().to_path_buf())
    }

    pub fn config_path() -> Option<PathBuf> {
        Self::config_dir().map(|d| d.join("config.toml"))
    }

    pub fn load() -> Result<Self, Box<dyn std::error::Error>> {
        let path = Self::config_path().ok_or("Cannot determine config directory")?;
        if path.exists() {
            let content = std::fs::read_to_string(&path)?;
            Ok(toml::from_str(&content)?)
        } else {
            let config = Config::default();
            config.save()?;
            Ok(config)
        }
    }

    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        let dir = Self::config_dir().ok_or("Cannot determine config directory")?;
        std::fs::create_dir_all(&dir)?;
        let path = dir.join("config.toml");
        let content = toml::to_string_pretty(self)?;
        std::fs::write(&path, content)?;
        Ok(())
    }

    /// 获取动画速度 (处理关联速度逻辑)
    pub fn get_animation_speeds(&self) -> (f64, f64) {
        if self.use_linked_speed {
            (self.effect_speed, self.effect_speed)
        } else {
            (self.trail_speed, self.click_speed)
        }
    }

    /// 解析 RGB 颜色字符串 "R,G,B" 到 [u8; 3]
    pub fn parse_color(&self) -> [u8; 3] {
        parse_rgb(&self.particle_color)
    }
}

/// 解析 "R,G,B" 格式的字符串到 [u8; 3]
pub fn parse_rgb(s: &str) -> [u8; 3] {
    let parts: Vec<u8> = s
        .split(',')
        .filter_map(|p| p.trim().parse().ok())
        .collect();
    if parts.len() == 3 {
        [parts[0], parts[1], parts[2]]
    } else {
        [45, 175, 255]
    }
}

/// 计算环结束颜色 (对应原 JS ringsEndColorFromRgb)
pub fn rings_end_color(r: u8, g: u8, b: u8) -> [u8; 3] {
    [
        ((r as f64 + 255.0 * 2.0) / 3.0).round() as u8,
        ((g as f64 + 255.0 * 2.0) / 3.0).round() as u8,
        ((b as f64 + 255.0 * 2.0) / 3.0).round() as u8,
    ]
}

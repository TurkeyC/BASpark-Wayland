use crate::config::Config;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

#[derive(Debug, Clone)]
pub enum InputEvent {
    MouseDown {
        button: MouseButton,
        x: f64,
        y: f64,
    },
    MouseUp {
        button: MouseButton,
    },
    MouseMove {
        x: f64,
        y: f64,
    },
}

pub struct CursorTracker {
    pub x: f64,
    pub y: f64,
    pub is_down: bool,
    pub last_button: Option<MouseButton>,
    sensitivity: f64,
    screen_w: f64,
    screen_h: f64,
    move_count: u64,
    max_x: f64,
    max_y: f64,
    min_x: f64,
    min_y: f64,
    calibrated: bool,
    /// 相对屏幕中心的比例偏移，用于 align 到实际鼠标位置
    offset_x: f64,
    offset_y: f64,
}

impl CursorTracker {
    pub fn new(config: &Config, screen_w: f64, screen_h: f64) -> Self {
        // 用 config 的 cursor_start 值，缩放到当前屏幕尺寸
        let cx = config.cursor_start_x * screen_w / 960.0;
        let cy = config.cursor_start_y * screen_h / 960.0;
        let cx = cx.clamp(0.0, screen_w);
        let cy = cy.clamp(0.0, screen_h);
        log::info!("Cursor start: ({:.0}, {:.0}) (screen {}x{})", cx, cy, screen_w, screen_h);
        Self {
            x: cx,
            y: cy,
            is_down: false,
            last_button: None,
            sensitivity: config.input_sensitivity,
            screen_w,
            screen_h,
            move_count: 0,
            max_x: cx,
            max_y: cy,
            min_x: cx,
            min_y: cy,
            calibrated: false,
            offset_x: 0.0,
            offset_y: 0.0,
        }
    }

    /// 中键重置：将当前追踪位置设为屏幕中心
    pub fn recenter(&mut self) {
        self.x = self.screen_w / 2.0 + self.offset_x;
        self.y = self.screen_h / 2.0 + self.offset_y;
        self.reset_bounds();
        self.calibrated = false;
        log::info!("Cursor recentered to ({:.0}, {:.0})", self.x, self.y);
    }

    pub fn apply_delta(&mut self, dx: f64, dy: f64) {
        self.move_count += 1;
        self.x += dx * self.sensitivity;
        self.y += dy * self.sensitivity;
        self.max_x = self.max_x.max(self.x);
        self.max_y = self.max_y.max(self.y);
        self.min_x = self.min_x.min(self.x);
        self.min_y = self.min_y.min(self.y);
        self.auto_calibrate();
        self.clamp();
    }

    fn clamp(&mut self) {
        self.x = self.x.clamp(0.0, self.screen_w);
        self.y = self.y.clamp(0.0, self.screen_h);
    }

    fn auto_calibrate(&mut self) {
        if self.calibrated || self.move_count < 20 {
            return;
        }

        let range_x = self.max_x - self.min_x;
        let range_y = self.max_y - self.min_y;
        let max_range = range_x.max(range_y);
        let screen_diag = self.screen_w.max(self.screen_h);

        if max_range > screen_diag * 3.0 {
            self.sensitivity *= 0.5;
            self.reset_bounds();
            log::info!("cal: overshoot {}, sens -> {:.4}", max_range as i32, self.sensitivity);
        } else if max_range > screen_diag * 0.25 {
            self.calibrated = true;
            log::info!("cal: locked at sens {:.4} (range {:.0}/{:.0})", self.sensitivity, range_x, range_y);
        } else if max_range < screen_diag * 0.04 && self.move_count > 60 {
            self.sensitivity *= 4.0;
            self.reset_bounds();
            log::info!("cal: undershoot {}, sens -> {:.4}", max_range as i32, self.sensitivity);
        }
    }

    fn reset_bounds(&mut self) {
        self.max_x = self.x;
        self.max_y = self.y;
        self.min_x = self.x;
        self.min_y = self.y;
        self.move_count = 0;
    }

    pub fn set_button(&mut self, button: MouseButton, pressed: bool) {
        if pressed {
            self.is_down = true;
            self.last_button = Some(button);
        } else {
            self.is_down = false;
        }
    }

    pub fn to_event_on_click(&self, button: MouseButton, pressed: bool) -> InputEvent {
        if pressed {
            InputEvent::MouseDown {
                button,
                x: self.x,
                y: self.y,
            }
        } else {
            InputEvent::MouseUp { button }
        }
    }

    pub fn to_move_event(&self) -> InputEvent {
        InputEvent::MouseMove {
            x: self.x,
            y: self.y,
        }
    }
}

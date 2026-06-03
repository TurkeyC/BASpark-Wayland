//! 输入事件类型定义 — Wayland wl_pointer 事件将通过 overlay 收集

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

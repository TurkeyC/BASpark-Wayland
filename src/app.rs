use crate::config::Config;
use crate::input::{InputEvent, MouseButton};
use crate::overlay::Overlay;
use crate::renderer::ParticleEngine;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut config = Config::load()?;

    write_pid_file()?;
    log::info!("BASpark v{} starting...", env!("CARGO_PKG_VERSION"));

    // 初始化
    let mut overlay = Overlay::new(&config)?;
    overlay.init()?;
    let mut engine = ParticleEngine::new(&config);

    // 调整引擎大小
    resize_engine(&mut engine, &mut overlay);

    // 信号处理
    let term = Arc::new(AtomicBool::new(false));
    signal_hook::flag::register(signal_hook::consts::SIGTERM, Arc::clone(&term))?;
    signal_hook::flag::register(signal_hook::consts::SIGINT, Arc::clone(&term))?;
    let hup = Arc::new(AtomicBool::new(false));
    signal_hook::flag::register(signal_hook::consts::SIGHUP, Arc::clone(&hup))?;

    log::info!("BASpark is running. Press Ctrl+C to stop.");

    let mut frame_timer = Instant::now();
    let target_frame = std::time::Duration::from_secs_f64(1.0 / config.trail_refresh_hz as f64);

    // 主循环
    loop {
        // 1. 信号检查
        if term.load(Ordering::Relaxed) {
            break;
        }
        if hup.swap(false, Ordering::Relaxed) {
            if let Ok(new_config) = Config::load() {
                config = new_config;
            }
        }

        // 2. Dispatch Wayland 事件
        overlay.poll_events(!engine.has_work())?;
        // 恢复穿透超时后的 input region
        overlay.check_passthrough();

        // 3. 获取并路由输入事件
        let input_events = overlay.take_input_events();
        for ev in &input_events {
            match ev {
                InputEvent::MouseDown { button, x, y } => {
                    if should_trigger(&config, *button) {
                        engine.on_mouse_down(*x, *y, &config);
                    }
                }
                InputEvent::MouseMove { x, y } => {
                    if overlay.is_button_down(MouseButton::Left)
                        || overlay.is_button_down(MouseButton::Right)
                        || config.enable_always_trail
                    {
                        engine.on_mouse_move(*x, *y, &config);
                    }
                }
                InputEvent::MouseUp { .. } => {
                    engine.on_mouse_up();
                }
            }
        }

        // 4. 渲染
        let now = Instant::now();
        if engine.has_work() && now.duration_since(frame_timer) >= target_frame {
            frame_timer = now;
            let t0 = Instant::now();
            if let Some(pixels) = engine.render_frame(&config) {
                let t1 = Instant::now();
                render_to_overlay(pixels, &mut overlay);
                let t2 = Instant::now();
                let render_ms = (t1 - t0).as_secs_f64() * 1000.0;
                let overlay_ms = (t2 - t1).as_secs_f64() * 1000.0;
                eprintln!("[FPS] render={:.1}ms overlay={:.1}ms total={:.1}ms",
                    render_ms, overlay_ms, render_ms + overlay_ms);
            }
        }

        // 5. 空闲重置
        if !engine.has_work() {
            frame_timer = Instant::now();
        }
    }

    cleanup_pid_file();
    log::info!("BASpark stopped.");
    Ok(())
}

fn should_trigger(config: &Config, button: MouseButton) -> bool {
    if !config.is_effect_enabled {
        return false;
    }
    match config.click_trigger {
        crate::config::ClickTrigger::Left => button == MouseButton::Left,
        crate::config::ClickTrigger::Right => button == MouseButton::Right,
        crate::config::ClickTrigger::Both => {
            button == MouseButton::Left || button == MouseButton::Right
        }
    }
}

fn resize_engine(engine: &mut ParticleEngine, overlay: &mut Overlay) {
    let surfaces = overlay.surfaces_mut();
    if let Some(surface) = surfaces.first() {
        engine.resize(surface.width as f64, surface.height as f64);
    }
}

fn render_to_overlay(pixels: &[u8], overlay: &mut Overlay) {
    for surface in overlay.surfaces_mut() {
        let buf = surface.pixels_mut();
        let pixel_count = buf.len().min(pixels.len()) / 4;

        for i in 0..pixel_count {
            let j = i * 4;
            let premul_b = pixels[j] as u32;
            let premul_g = pixels[j + 1] as u32;
            let premul_r = pixels[j + 2] as u32;
            let a = pixels[j + 3] as u32;

            let (r, g, b) = if a == 0 {
                (0u32, 0u32, 0u32)
            } else {
                (premul_r * 255 / a, premul_g * 255 / a, premul_b * 255 / a)
            };

            buf[j] = b as u8;
            buf[j + 1] = g as u8;
            buf[j + 2] = r as u8;
            buf[j + 3] = a as u8;
        }
    }

    overlay.commit_and_swap();
}

fn write_pid_file() -> Result<(), Box<dyn std::error::Error>> {
    let path = crate::pid_file_path()
        .unwrap_or_else(|| std::path::PathBuf::from("/tmp/baspark.pid"));
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, std::process::id().to_string())?;
    Ok(())
}

fn cleanup_pid_file() {
    if let Some(path) = crate::pid_file_path() {
        let _ = std::fs::remove_file(&path);
    }
}

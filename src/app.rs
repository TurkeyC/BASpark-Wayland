use crate::config::Config;
use crate::input::{EvdevMonitor, InputEvent, MouseButton, CursorTracker};
use crate::output::WaylandOutput;
use crate::renderer::ParticleEngine;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut config = Config::load()?;

    write_pid_file()?;
    log::info!("BASpark v{} starting...", env!("CARGO_PKG_VERSION"));

    let mut output = WaylandOutput::new()?;
    output.init()?;
    let mut engine = ParticleEngine::new(&config);
    let (screen_w, screen_h) = get_screen_size(&mut output);
    engine.resize(screen_w, screen_h);

    let cursor = CursorTracker::new(&config, screen_w, screen_h);
    let mut monitor = EvdevMonitor::new(cursor);
    monitor.discover();

    if !monitor.has_devices() {
        log::error!("No pointer input devices found.");
        log::error!("Make sure your user is in the 'input' group:");
        log::error!("  sudo usermod -a -G input $USER");
        return Err("No input devices available".into());
    }
    log::info!("Found {} pointer device(s)", monitor.device_count());

    let term = Arc::new(AtomicBool::new(false));
    signal_hook::flag::register(signal_hook::consts::SIGTERM, Arc::clone(&term))?;
    signal_hook::flag::register(signal_hook::consts::SIGINT, Arc::clone(&term))?;
    let hup = Arc::new(AtomicBool::new(false));
    signal_hook::flag::register(signal_hook::consts::SIGHUP, Arc::clone(&hup))?;

    log::info!("BASpark is running. Press Ctrl+C to stop.");

    let target_frame = std::time::Duration::from_secs_f64(1.0 / config.trail_refresh_hz as f64);
    let target_frame_us = target_frame.as_micros() as i64;
    let mut frame_timer = Instant::now();

    loop {
        if term.load(Ordering::Relaxed) { break; }
        if hup.swap(false, Ordering::Relaxed) {
            if let Ok(new_config) = Config::load() {
                config = new_config;
                log::info!("Config reloaded via SIGHUP");
            }
        }

        // 等待 evdev 输入或帧间隔超时
        let now = Instant::now();
        let elapsed_us = now.duration_since(frame_timer).as_micros() as i64;
        let wait_us = if engine.has_work() {
            (target_frame_us - elapsed_us).max(0).min(target_frame_us)
        } else {
            8_000 // 空闲时 8ms 轮询一次
        };

        if wait_us > 500 {
            monitor.wait_for_events(wait_us as i32);
        }

        // 处理 Wayland 事件
        let _ = output.poll_events(false);

        // 处理 evdev 输入
        let events = monitor.poll();
        for ev in &events {
            match ev {
                InputEvent::MouseDown { button, x, y } => {
                    log::trace!("MouseDown btn={:?} x={:.0} y={:.0}", button, x, y);
                    if *button == MouseButton::Middle {
                        monitor.cursor.recenter();
                        continue;
                    }
                    if should_trigger(&config, *button) {
                        engine.on_mouse_down(*x, *y, &config);
                    }
                }
                InputEvent::MouseMove { x, y } => {
                    if monitor.cursor.is_down || config.enable_always_trail {
                        engine.on_mouse_move(*x, *y, &config);
                    }
                }
                InputEvent::MouseUp { button } => {
                    if *button == MouseButton::Middle {
                        continue;
                    }
                    engine.on_mouse_up();
                }
            }
        }

        // 渲染
        let now = Instant::now();
        if engine.has_work() && now.duration_since(frame_timer) >= target_frame {
            frame_timer = now;
            if let Some(pixels) = engine.render_frame(&config) {
                render_to_overlay(pixels, &mut output);
            }
        }

        if !engine.has_work() {
            frame_timer = Instant::now();
        }
    }

    cleanup_pid_file();
    log::info!("BASpark stopped.");
    Ok(())
}

fn get_screen_size(output: &mut WaylandOutput) -> (f64, f64) {
    if let Some(s) = output.surfaces_mut().first() {
        (s.width as f64, s.height as f64)
    } else {
        (1920.0, 1080.0)
    }
}

fn should_trigger(config: &Config, button: MouseButton) -> bool {
    if !config.is_effect_enabled { return false; }
    match config.click_trigger {
        crate::config::ClickTrigger::Left => button == MouseButton::Left,
        crate::config::ClickTrigger::Right => button == MouseButton::Right,
        crate::config::ClickTrigger::Both => {
            button == MouseButton::Left || button == MouseButton::Right
        }
    }
}

fn render_to_overlay(pixels: &[u8], output: &mut WaylandOutput) {
    for surface in output.surfaces_mut() {
        let buf = surface.pixels_mut();
        let n = buf.len().min(pixels.len()) / 4;

        // tiny_skia Pixmap 是 RGBA premultiplied, Wayland ARGB8888 (LE) 是 BGRA non-premultiplied
        for i in 0..n {
            let j = i * 4;
            let pr = pixels[j] as u32;
            let pg = pixels[j + 1] as u32;
            let pb = pixels[j + 2] as u32;
            let a = pixels[j + 3] as u32;
            // 快速通道: 全透明或完全不透明时避免除法
            if a == 0 || a == 255 {
                buf[j]     = pb as u8;
                buf[j + 1] = pg as u8;
                buf[j + 2] = pr as u8;
                buf[j + 3] = a as u8;
            } else {
                buf[j]     = pb as u8;
                buf[j + 1] = (pg * 255 / a) as u8;
                buf[j + 2] = (pr * 255 / a) as u8;
                buf[j + 3] = a as u8;
            }
        }


    }
    output.commit_and_swap();
}

fn write_pid_file() -> Result<(), Box<dyn std::error::Error>> {
    let path = crate::pid_file_path()
        .unwrap_or_else(|| std::path::PathBuf::from("/tmp/baspark.pid"));
    if let Some(parent) = path.parent() { std::fs::create_dir_all(parent)?; }
    std::fs::write(&path, std::process::id().to_string())?;
    Ok(())
}

fn cleanup_pid_file() {
    if let Some(path) = crate::pid_file_path() {
        let _ = std::fs::remove_file(&path);
    }
}

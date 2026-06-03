mod app;
mod autostart;
mod config;
mod input;
mod overlay;
mod renderer;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "baspark", about = "Blue Archive style particle effects overlay")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the overlay daemon
    Start,
    /// Stop the running daemon
    Stop,
    /// Check daemon status
    Status,
    /// View or modify configuration
    Config {
        #[arg(long, help = "Set particle color (e.g. '45,175,255')")]
        color: Option<String>,
        #[arg(long, help = "Set effect scale (0.5-3.0)")]
        scale: Option<f64>,
        #[arg(long, help = "Set effect opacity (0.1-1.0)")]
        opacity: Option<f64>,
        #[arg(long, help = "Set trail speed (0.2-3.0)")]
        trail_speed: Option<f64>,
        #[arg(long, help = "Set click speed (0.2-3.0)")]
        click_speed: Option<f64>,
        #[arg(long, help = "Enable/disable linked speed")]
        linked_speed: Option<bool>,
        #[arg(long, help = "Enable/disable always trail")]
        always_trail: Option<bool>,
        #[arg(long, help = "Set refresh rate Hz (10-240)")]
        refresh_rate: Option<u32>,
        #[arg(long, help = "Enable/disable effect")]
        enabled: Option<bool>,
        #[arg(long, help = "Show current config")]
        show: bool,
    },
    /// Reload running daemon config (SIGHUP)
    Reload,
    /// Manage autostart
    Autostart {
        #[arg(long, help = "Enable autostart")]
        enable: bool,
        #[arg(long, help = "Disable autostart")]
        disable: bool,
    },
}

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("baspark=info")).init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Start => {
            log::info!("Starting BASpark...");
            if let Err(e) = app::run() {
                log::error!("Fatal error: {}", e);
                std::process::exit(1);
            }
        }
        Commands::Stop => {
            if let Err(e) = signal_daemon(nix::sys::signal::Signal::SIGTERM) {
                eprintln!("Failed to stop daemon: {}", e);
                std::process::exit(1);
            }
            println!("Daemon stopped.");
        }
        Commands::Status => {
            match pid_file_path().and_then(|p| read_pid(&p)) {
                Some(pid) => {
                    let running = nix::sys::signal::kill(
                        nix::unistd::Pid::from_raw(pid),
                        None,
                    )
                    .is_ok();
                    if running {
                        println!("BASpark is running (PID: {})", pid);
                    } else {
                        println!("BASpark is not running (stale PID file)");
                    }
                }
                None => println!("BASpark is not running."),
            }
        }
        Commands::Config {
            color,
            scale,
            opacity,
            trail_speed,
            click_speed,
            linked_speed,
            always_trail,
            refresh_rate,
            enabled,
            show,
        } => {
            let mut config = config::Config::load().unwrap_or_default();
            let mut changed = false;

            if let Some(v) = color {
                config.particle_color = v;
                changed = true;
            }
            if let Some(v) = scale {
                config.effect_scale = v.clamp(0.5, 3.0);
                changed = true;
            }
            if let Some(v) = opacity {
                config.effect_opacity = v.clamp(0.1, 1.0);
                changed = true;
            }
            if let Some(v) = trail_speed {
                config.trail_speed = v.clamp(0.2, 3.0);
                changed = true;
            }
            if let Some(v) = click_speed {
                config.click_speed = v.clamp(0.2, 3.0);
                changed = true;
            }
            if let Some(v) = linked_speed {
                config.use_linked_speed = v;
                changed = true;
            }
            if let Some(v) = always_trail {
                config.enable_always_trail = v;
                changed = true;
            }
            if let Some(v) = refresh_rate {
                config.trail_refresh_hz = v.clamp(10, 240);
                changed = true;
            }
            if let Some(v) = enabled {
                config.is_effect_enabled = v;
                changed = true;
            }

            if changed {
                config.save().unwrap_or_else(|e| {
                    eprintln!("Failed to save config: {}", e);
                });
                let _ = signal_daemon(nix::sys::signal::Signal::SIGHUP);
            }

            if show || !changed {
                print_config(&config);
            }
        }
        Commands::Reload => {
            if let Err(e) = signal_daemon(nix::sys::signal::Signal::SIGHUP) {
                eprintln!("Failed to reload daemon: {}", e);
                std::process::exit(1);
            }
            println!("Reload signal sent.");
        }
        Commands::Autostart { enable, disable } => {
            if enable {
                autostart::enable().unwrap_or_else(|e| {
                    eprintln!("Failed to enable autostart: {}", e);
                });
                println!("Autostart enabled.");
            } else if disable {
                autostart::disable().unwrap_or_else(|e| {
                    eprintln!("Failed to disable autostart: {}", e);
                });
                println!("Autostart disabled.");
            } else {
                let enabled = autostart::is_enabled();
                println!(
                    "Autostart: {}",
                    if enabled { "enabled" } else { "disabled" }
                );
            }
        }
    }
}

fn print_config(config: &config::Config) {
    println!("Current configuration:");
    println!("  particle_color      = {}", config.particle_color);
    println!("  is_effect_enabled   = {}", config.is_effect_enabled);
    println!("  effect_scale        = {}", config.effect_scale);
    println!("  effect_opacity      = {}", config.effect_opacity);
    println!("  use_linked_speed    = {}", config.use_linked_speed);
    println!("  effect_speed        = {}", config.effect_speed);
    println!("  trail_speed         = {}", config.trail_speed);
    println!("  click_speed         = {}", config.click_speed);
    println!("  trail_refresh_hz    = {}", config.trail_refresh_hz);
    println!("  enable_always_trail = {}", config.enable_always_trail);
    println!("  click_trigger       = {:?}", config.click_trigger);
    println!("  input_sensitivity   = {}", config.input_sensitivity);
}

pub(crate) fn pid_file_path() -> Option<std::path::PathBuf> {
    std::env::var("XDG_RUNTIME_DIR")
        .ok()
        .map(|d| std::path::PathBuf::from(d).join("baspark.pid"))
        .or_else(|| Some(std::path::PathBuf::from("/tmp/baspark.pid")))
}

fn read_pid(path: &std::path::Path) -> Option<i32> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| s.trim().parse().ok())
}

fn signal_daemon(sig: nix::sys::signal::Signal) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(path) = pid_file_path() {
        if let Some(pid) = read_pid(&path) {
            nix::sys::signal::kill(nix::unistd::Pid::from_raw(pid), Some(sig))?;
        }
    }
    Ok(())
}

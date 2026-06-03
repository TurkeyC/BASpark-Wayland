//! XDG autostart 和 systemd user service 管理

use std::path::PathBuf;

fn autostart_dir() -> Option<PathBuf> {
    directories::UserDirs::new()
        .map(|d| d.home_dir().join(".config").join("autostart"))
}

fn desktop_file_path() -> Option<PathBuf> {
    autostart_dir().map(|d| d.join("baspark.desktop"))
}

pub fn enable() -> Result<(), Box<dyn std::error::Error>> {
    let dir = autostart_dir().ok_or("Cannot determine autostart directory")?;
    std::fs::create_dir_all(&dir)?;

    let exe = std::env::current_exe()
        .unwrap_or_else(|_| PathBuf::from("baspark"));

    let content = format!(
        r#"[Desktop Entry]
Type=Application
Name=BASpark
Comment=Blue Archive style particle effects
Exec={} start
X-GNOME-Autostart-enabled=true
StartupNotify=false
NoDisplay=true
"#,
        exe.display()
    );

    std::fs::write(desktop_file_path().unwrap(), content)?;
    Ok(())
}

pub fn disable() -> Result<(), Box<dyn std::error::Error>> {
    if let Some(path) = desktop_file_path() {
        if path.exists() {
            std::fs::remove_file(&path)?;
        }
    }
    Ok(())
}

pub fn is_enabled() -> bool {
    desktop_file_path()
        .map(|p| p.exists())
        .unwrap_or(false)
}

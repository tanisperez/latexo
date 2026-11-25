//! "Start with the session": a desktop entry in `~/.config/autostart` on Linux (the XDG autostart
//! standard every desktop follows) and a value under `HKCU\...\Run` on Windows, written with the
//! `reg` tool that every Windows has, so no registry crate is needed.

use std::path::{Path, PathBuf};

/// The name of the autostart entry, also the Windows `Run` value.
const ENTRY: &str = "codes.tanis.latexo.desktop";
#[cfg(windows)]
const RUN_KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";

/// The desktop entry that starts `exe`.
pub fn desktop_entry(exe: &Path) -> String {
    format!(
        "[Desktop Entry]\nType=Application\nName=Latexo\nExec=\"{}\"\nIcon=codes.tanis.latexo\nX-GNOME-Autostart-enabled=true\nNoDisplay=false\n",
        exe.display()
    )
}

fn autostart_file() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| std::env::home_dir().map(|home| home.join(".config")))?;
    Some(base.join("autostart").join(ENTRY))
}

/// Turns starting with the session on or off. Errors are logged and returned as text for a
/// notification.
pub fn set_enabled(enabled: bool) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|error| error.to_string())?;
    let result = if cfg!(windows) {
        set_windows(enabled, &exe)
    } else {
        set_linux(enabled, &exe)
    };
    match &result {
        Ok(()) => log::info!("start with the session: {enabled}"),
        Err(error) => log::warn!("could not change the start with the session: {error}"),
    }
    result
}

fn set_linux(enabled: bool, exe: &Path) -> Result<(), String> {
    let file = autostart_file().ok_or("no config folder")?;
    if enabled {
        if let Some(parent) = file.parent() {
            std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        std::fs::write(&file, desktop_entry(exe)).map_err(|error| error.to_string())
    } else {
        match std::fs::remove_file(&file) {
            Err(error) if error.kind() != std::io::ErrorKind::NotFound => Err(error.to_string()),
            _ => Ok(()),
        }
    }
}

#[cfg(windows)]
fn set_windows(enabled: bool, exe: &Path) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    // CREATE_NO_WINDOW: no console flashes for a moment.
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let mut command = std::process::Command::new("reg");
    if enabled {
        command.args([
            "add",
            RUN_KEY,
            "/v",
            "Latexo",
            "/t",
            "REG_SZ",
            "/d",
            &format!("\"{}\"", exe.display()),
            "/f",
        ]);
    } else {
        command.args(["delete", RUN_KEY, "/v", "Latexo", "/f"]);
    }
    let status = command
        .creation_flags(CREATE_NO_WINDOW)
        .status()
        .map_err(|error| error.to_string())?;
    // Deleting a value that is not there fails, and that is fine.
    if status.success() || !enabled {
        Ok(())
    } else {
        Err(format!("reg exited with {status}"))
    }
}

#[cfg(not(windows))]
fn set_windows(_enabled: bool, _exe: &Path) -> Result<(), String> {
    Err("not Windows".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_entry_runs_the_executable() {
        let entry = desktop_entry(Path::new("/opt/latexo/latexo"));
        assert!(entry.contains("Exec=\"/opt/latexo/latexo\""));
        assert!(entry.starts_with("[Desktop Entry]"));
    }
}

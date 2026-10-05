//! Installs an update on Windows and Linux, the counterpart of the Mac's DMG swap. Only
//! files from Parla's own GitHub releases ever get here (the caller checks the address).

use std::{path::Path, process::Command};

/// Whether this copy of Parla can install an update itself, or the user has to.
pub fn supported() -> Result<(), String> {
    #[cfg(target_os = "linux")]
    if std::env::var_os("APPIMAGE").is_none() {
        return Err("This copy of Parla was installed by your package manager. Download the update instead.".into());
    }
    Ok(())
}

/// What the update file should be called, from the end of its address.
pub fn file_name(url: &str) -> Result<String, String> {
    let name = url.rsplit('/').next().unwrap_or_default();
    let expected = if cfg!(target_os = "windows") { "-setup.exe" } else { ".AppImage" };
    if name.is_empty() || !name.ends_with(expected) || name.contains(['\\', '/']) {
        return Err("That isn't a Parla release for this computer.".into());
    }
    Ok(name.to_string())
}

/// Starts installing `file` once Parla has quit. On Windows that is the installer,
/// which replaces the old version itself; on Linux the AppImage is swapped in place and
/// opened again.
pub fn install(file: &Path, pid: u32) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        let _ = pid;
        // /P shows the installer's progress without asking anything.
        Command::new(file)
            .arg("/P")
            .spawn()
            .map(|_| ())
            .map_err(|e| format!("Couldn't start the installer: {e}"))
    }
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::fs::PermissionsExt;
        let target = std::env::var_os("APPIMAGE").ok_or("Parla isn't running as an AppImage.")?;
        std::fs::set_permissions(file, std::fs::Permissions::from_mode(0o755))
            .map_err(|e| e.to_string())?;
        Command::new("/bin/sh")
            .arg("-c")
            .arg(
                r#"while kill -0 "$1" 2>/dev/null; do sleep 0.2; done
mv -f "$2" "$3" && exec "$3""#,
            )
            .arg("parla-update")
            .arg(pid.to_string())
            .arg(file)
            .arg(target)
            .spawn()
            .map(|_| ())
            .map_err(|e| format!("Couldn't start the update: {e}"))
    }
    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
    {
        let _ = (file, pid);
        Err("Updates install themselves only on Windows and Linux.".into())
    }
}

/// Opens a web address in the default browser.
pub fn open_url(url: &str) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    let mut command = {
        let mut command = Command::new("explorer");
        command.arg(url);
        command
    };
    #[cfg(not(target_os = "windows"))]
    let mut command = {
        let mut command = Command::new("xdg-open");
        command.arg(url);
        command
    };
    command.spawn().map(|_| ()).map_err(|e| e.to_string())
}

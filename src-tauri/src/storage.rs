use std::{fs, path::PathBuf};

use serde::{de::DeserializeOwned, Serialize};
use tauri::{AppHandle, Manager};

pub fn path(app: &AppHandle, file: &str) -> PathBuf {
    app.path()
        .app_data_dir()
        .map(|dir| dir.join(file))
        .unwrap_or_else(|_| PathBuf::from(file))
}

pub fn read<T: DeserializeOwned + Default>(path: &PathBuf) -> T {
    fs::read_to_string(path)
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default()
}

pub fn write<T: Serialize>(path: &PathBuf, value: &T) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?;
    fs::write(path, json).map_err(|e| e.to_string())
}

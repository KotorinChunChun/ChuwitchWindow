use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub rotate_cw_hotkey: String,
    pub rotate_ccw_hotkey: String,
    pub undo_hotkey: String,
    pub swap_target_modifiers: String,
    pub primary_monitor_id: Option<String>,
    pub swap_within_groups: bool,
    pub monitor_order: Vec<String>,
    pub ignore_fullscreen: bool,
    pub run_on_startup: bool,
    pub run_as_admin: bool,
    pub monitor_groups: HashMap<String, u8>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            rotate_cw_hotkey: "Win+Ctrl+Alt+Right".to_string(),
            rotate_ccw_hotkey: "Win+Ctrl+Alt+Left".to_string(),
            undo_hotkey: "Win+Ctrl+Alt+Z".to_string(),
            swap_target_modifiers: "Win+Ctrl+Alt".to_string(),
            primary_monitor_id: None,
            swap_within_groups: false,
            monitor_order: Vec::new(),
            ignore_fullscreen: true,
            run_on_startup: false,
            run_as_admin: false,
            monitor_groups: HashMap::new(),
        }
    }
}

pub fn get_config_path() -> Option<PathBuf> {
    let proj_dirs = ProjectDirs::from("com", "kotorichun", "chuwitchwindow")?;
    let config_dir = proj_dirs.config_dir();
    let _ = fs::create_dir_all(config_dir);
    Some(config_dir.join("config.json"))
}

pub fn load_config() -> AppConfig {
    if let Some(path) = get_config_path() {
        if let Ok(content) = fs::read_to_string(&path) {
            if let Ok(config) = serde_json::from_str(&content) {
                return config;
            }
        }
    }
    AppConfig::default()
}

pub fn save_config(config: &AppConfig) {
    if let Some(path) = get_config_path() {
        if let Ok(content) = serde_json::to_string_pretty(config) {
            let _ = fs::write(path, content);
        }
    }
}

pub fn reset_config() {
    if let Some(path) = get_config_path() {
        let _ = fs::remove_file(path);
    }
}

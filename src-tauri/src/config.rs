use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

/// アプリケーション設定
/// `#[serde(default)]` により、既存の config.json に新フィールドがなくても
/// デフォルト値で補完されるため前方互換性を維持
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    // --- v0.1 既存フィールド ---
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

    // --- v0.2 新規フィールド ---
    /// ピン留めトグルのショートカットキー (SPEC 2-1)
    pub pin_hotkey: String,
    /// 一斉退避（エスケープ）のショートカットキー (SPEC 2-3)
    pub escape_hotkey: String,
    /// 一極集中（ギャザー）のショートカットキー (SPEC 2-3)
    pub gather_hotkey: String,
    /// 自動整列のショートカットキー (SPEC 2-6)
    pub arrange_hotkey: String,
    /// 最小化ウィンドウを移動対象から除外するか (SPEC 2-5)
    pub exclude_minimized: bool,
    /// 除外ルール（JSONフォーマットの文字列） (SPEC 2-2)
    pub exclusion_rules: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            // v0.1 既存
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
            // v0.2 新規
            pin_hotkey: "Win+Ctrl+Alt+P".to_string(),
            escape_hotkey: "Win+Ctrl+Alt+E".to_string(),
            gather_hotkey: "Win+Ctrl+Alt+G".to_string(),
            arrange_hotkey: "Win+Ctrl+Alt+A".to_string(),
            exclude_minimized: true,
            exclusion_rules: "[]".to_string(),
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

#[cfg(test)]
mod tests {
    use super::*;

    // ---------------------------------------------------------
    // デフォルト値のテスト
    // ---------------------------------------------------------
    #[test]
    fn test_default_config_hotkeys() {
        let cfg = AppConfig::default();
        assert_eq!(cfg.rotate_cw_hotkey, "Win+Ctrl+Alt+Right");
        assert_eq!(cfg.rotate_ccw_hotkey, "Win+Ctrl+Alt+Left");
        assert_eq!(cfg.undo_hotkey, "Win+Ctrl+Alt+Z");
        assert_eq!(cfg.pin_hotkey, "Win+Ctrl+Alt+P");
        assert_eq!(cfg.escape_hotkey, "Win+Ctrl+Alt+E");
        assert_eq!(cfg.gather_hotkey, "Win+Ctrl+Alt+G");
        assert_eq!(cfg.arrange_hotkey, "Win+Ctrl+Alt+A");
    }

    #[test]
    fn test_default_config_flags() {
        let cfg = AppConfig::default();
        assert!(cfg.ignore_fullscreen);
        assert!(!cfg.run_on_startup);
        assert!(!cfg.run_as_admin);
        assert!(!cfg.swap_within_groups);
        assert!(cfg.exclude_minimized);
    }

    #[test]
    fn test_default_config_collections_empty() {
        let cfg = AppConfig::default();
        assert!(cfg.monitor_order.is_empty());
        assert!(cfg.monitor_groups.is_empty());
        assert!(cfg.primary_monitor_id.is_none());
    }

    // ---------------------------------------------------------
    // シリアライズ → デシリアライズの往復テスト
    // ---------------------------------------------------------
    #[test]
    fn test_serialize_deserialize_roundtrip() {
        let original = AppConfig::default();
        let json = serde_json::to_string(&original).expect("serialize failed");
        let restored: AppConfig = serde_json::from_str(&json).expect("deserialize failed");
        // 主要フィールドが一致することを確認
        assert_eq!(restored.rotate_cw_hotkey, original.rotate_cw_hotkey);
        assert_eq!(restored.undo_hotkey, original.undo_hotkey);
        assert_eq!(restored.pin_hotkey, original.pin_hotkey);
        assert_eq!(restored.exclude_minimized, original.exclude_minimized);
    }

    #[test]
    fn test_deserialize_partial_json_uses_defaults() {
        // 一部フィールドだけ持つ JSON を読み込んだとき、欠けているフィールドはデフォルト値になる
        let json = r#"{"rotate_cw_hotkey": "Win+Right"}"#;
        let cfg: AppConfig = serde_json::from_str(json).expect("deserialize failed");
        assert_eq!(cfg.rotate_cw_hotkey, "Win+Right");
        // これは `#[serde(default)]` から補完されるはず
        assert_eq!(cfg.undo_hotkey, "Win+Ctrl+Alt+Z");
        assert_eq!(cfg.pin_hotkey, "Win+Ctrl+Alt+P");
    }

    #[test]
    fn test_exclusion_rules_default_is_empty_array() {
        let cfg = AppConfig::default();
        // デフォルトの exclusion_rules は "[]"（空配列のJSON文字列）
        assert_eq!(cfg.exclusion_rules, "[]");
    }

    #[test]
    fn test_serialize_contains_all_fields() {
        let cfg = AppConfig::default();
        let json = serde_json::to_string(&cfg).expect("serialize failed");
        // 主要フィールドがシリアライズ結果に含まれていることを確認
        assert!(json.contains("rotate_cw_hotkey"));
        assert!(json.contains("pin_hotkey"));
        assert!(json.contains("exclusion_rules"));
        assert!(json.contains("exclude_minimized"));
    }
}

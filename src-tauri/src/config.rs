use crate::rule::{parse_rules, RuleCondition, RuleNode};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

pub const CURRENT_RULE_SCHEMA_VERSION: u32 = 1;

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
    pub run_on_startup: bool,
    pub run_as_admin: bool,
    pub monitor_groups: HashMap<String, u8>,
    pub middle_drag_enabled: bool,
    pub alt_drag_follow_neighbors: bool,
    pub drag_snap_to_work_area: bool,
    pub drag_resize_across_monitors: bool,

    // --- v0.2 新規フィールド ---
    /// ピン留めトグルのショートカットキー (SPEC 2-1)
    pub pin_hotkey: String,
    /// 一斉退避（エスケープ）のショートカットキー (SPEC 2-3)
    pub escape_hotkey: String,
    /// 一極集中（ギャザー）のショートカットキー (SPEC 2-3)
    pub gather_hotkey: String,
    /// 自動整列のショートカットキー (SPEC 2-6)
    pub arrange_hotkey: String,
    /// 除外ルールのマイグレーション管理用バージョン
    pub rule_schema_version: u32,
    /// 除外ルール（JSONフォーマットの文字列）
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
            run_on_startup: false,
            run_as_admin: false,
            monitor_groups: HashMap::new(),
            middle_drag_enabled: false,
            alt_drag_follow_neighbors: false,
            drag_snap_to_work_area: true,
            drag_resize_across_monitors: true,
            // v0.2 新規
            pin_hotkey: "Win+Ctrl+Alt+P".to_string(),
            escape_hotkey: "Win+Ctrl+Alt+E".to_string(),
            gather_hotkey: "Win+Ctrl+Alt+G".to_string(),
            arrange_hotkey: "Win+Ctrl+Alt+A".to_string(),
            rule_schema_version: CURRENT_RULE_SCHEMA_VERSION,
            exclusion_rules: serialize_rules(&default_exclusion_rules()),
        }
    }
}

fn fullscreen_exclusion_rule() -> RuleNode {
    RuleNode::Rule {
        conditions: vec![
            RuleCondition {
                field: "is_fullscreen".to_string(),
                operator: "equals".to_string(),
                value: "true".to_string(),
            },
            RuleCondition {
                field: "process_name".to_string(),
                operator: "not_wildcard".to_string(),
                value: "mstsc*.exe".to_string(),
            },
        ],
    }
}

fn minimized_exclusion_rule() -> RuleNode {
    RuleNode::Rule {
        conditions: vec![RuleCondition {
            field: "is_minimized".to_string(),
            operator: "equals".to_string(),
            value: "true".to_string(),
        }],
    }
}

fn empty_exclusion_rules() -> RuleNode {
    RuleNode::Group {
        match_type: "OR".to_string(),
        children: Vec::new(),
    }
}

fn default_exclusion_rules() -> RuleNode {
    RuleNode::Group {
        match_type: "OR".to_string(),
        children: vec![fullscreen_exclusion_rule(), minimized_exclusion_rule()],
    }
}

fn serialize_rules(rules: &RuleNode) -> String {
    serde_json::to_string_pretty(rules).unwrap_or_else(|_| "[]".to_string())
}

/// 旧トグル設定を、同じ意味を持つ通常の除外ルールへ一度だけ移行する。
fn migrate_legacy_exclusion_rules(
    current_json: &str,
    ignore_fullscreen: bool,
    exclude_minimized: bool,
) -> String {
    let parsed = parse_rules(current_json);
    let mut children = match parsed {
        RuleNode::Group { match_type, children } if match_type.eq_ignore_ascii_case("OR") => {
            children
        }
        RuleNode::Group { children, .. } if children.is_empty() => Vec::new(),
        other => vec![other],
    };

    let mut append_unique = |rule: RuleNode| {
        if !children.contains(&rule) {
            children.push(rule);
        }
    };

    if ignore_fullscreen {
        append_unique(fullscreen_exclusion_rule());
    }
    if exclude_minimized {
        append_unique(minimized_exclusion_rule());
    }

    serialize_rules(&RuleNode::Group {
        match_type: "OR".to_string(),
        children,
    })
}

/// 設定JSONを読み込み、旧フルスクリーン／最小化トグルを必要に応じて移行する。
pub fn parse_config_json(content: &str) -> Result<(AppConfig, bool), serde_json::Error> {
    let raw: Value = serde_json::from_str(content)?;
    let raw_version = raw
        .get("rule_schema_version")
        .and_then(Value::as_u64)
        .unwrap_or(0) as u32;
    let has_legacy_filters = raw.get("ignore_fullscreen").is_some()
        || raw.get("exclude_minimized").is_some();
    let had_exclusion_rules = raw.get("exclusion_rules").is_some();
    let ignore_fullscreen = raw
        .get("ignore_fullscreen")
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let exclude_minimized = raw
        .get("exclude_minimized")
        .and_then(Value::as_bool)
        .unwrap_or(true);

    let mut config: AppConfig = serde_json::from_value(raw)?;
    let mut migrated = false;

    if raw_version < CURRENT_RULE_SCHEMA_VERSION {
        if has_legacy_filters {
            let base_rules = if had_exclusion_rules {
                config.exclusion_rules.clone()
            } else {
                serialize_rules(&empty_exclusion_rules())
            };
            config.exclusion_rules = migrate_legacy_exclusion_rules(
                &base_rules,
                ignore_fullscreen,
                exclude_minimized,
            );
        }
        config.rule_schema_version = CURRENT_RULE_SCHEMA_VERSION;
        migrated = true;
    }

    Ok((config, migrated))
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
            if let Ok((config, migrated)) = parse_config_json(&content) {
                if migrated {
                    save_config(&config);
                }
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
    fn test_default_config_flags_are_integrated_into_rules() {
        let cfg = AppConfig::default();
        assert!(!cfg.run_on_startup);
        assert!(!cfg.run_as_admin);
        assert!(!cfg.swap_within_groups);
        assert_eq!(cfg.rule_schema_version, CURRENT_RULE_SCHEMA_VERSION);

        let RuleNode::Group { children, .. } = parse_rules(&cfg.exclusion_rules) else {
            panic!("既定の除外ルールはORグループである必要があります");
        };
        assert!(children.contains(&fullscreen_exclusion_rule()));
        assert!(children.contains(&minimized_exclusion_rule()));
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
        assert_eq!(restored.rule_schema_version, original.rule_schema_version);
        assert_eq!(restored.exclusion_rules, original.exclusion_rules);
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
    fn test_exclusion_rules_default_contains_window_state_rules() {
        let cfg = AppConfig::default();
        assert!(cfg.exclusion_rules.contains("is_fullscreen"));
        assert!(cfg.exclusion_rules.contains("is_minimized"));
        assert!(cfg.exclusion_rules.contains("mstsc*.exe"));
    }

    #[test]
    fn test_serialize_contains_all_fields() {
        let cfg = AppConfig::default();
        let json = serde_json::to_string(&cfg).expect("serialize failed");
        // 主要フィールドがシリアライズ結果に含まれていることを確認
        assert!(json.contains("rotate_cw_hotkey"));
        assert!(json.contains("pin_hotkey"));
        assert!(json.contains("exclusion_rules"));
        assert!(json.contains("rule_schema_version"));
        assert!(!json.contains("ignore_fullscreen"));
        assert!(!json.contains("exclude_minimized"));
    }

    #[test]
    fn test_legacy_filters_are_migrated_and_existing_rules_are_preserved() {
        let json = r#"{
            "rotate_cw_hotkey": "Win+Right",
            "ignore_fullscreen": true,
            "exclude_minimized": true,
            "exclusion_rules": "[{\"match_type\":\"AND\",\"conditions\":[{\"field\":\"process_name\",\"operator\":\"equals\",\"value\":\"chrome.exe\"}]}]"
        }"#;

        let (config, migrated) = parse_config_json(json).expect("migration failed");
        assert!(migrated);
        assert_eq!(config.rule_schema_version, CURRENT_RULE_SCHEMA_VERSION);

        let RuleNode::Group { children, .. } = parse_rules(&config.exclusion_rules) else {
            panic!("移行後はORグループである必要があります");
        };
        assert_eq!(children.len(), 3);
        assert!(children.contains(&fullscreen_exclusion_rule()));
        assert!(children.contains(&minimized_exclusion_rule()));
    }

    #[test]
    fn test_disabled_legacy_filters_do_not_create_rules() {
        let json = r#"{
            "ignore_fullscreen": false,
            "exclude_minimized": false,
            "exclusion_rules": "[]"
        }"#;

        let (config, migrated) = parse_config_json(json).expect("migration failed");
        assert!(migrated);
        let RuleNode::Group { children, .. } = parse_rules(&config.exclusion_rules) else {
            panic!("移行後はORグループである必要があります");
        };
        assert!(children.is_empty());
    }
}

use crate::window::WindowInfo;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RuleCondition {
    pub field: String,    // "process_name", "class_name", "title", "style", "ex_style"
    pub operator: String, // "equals", "contains", "starts_with", "ends_with"
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum RuleNode {
    #[serde(rename = "group")]
    Group {
        match_type: String, // "AND" または "OR"
        children: Vec<RuleNode>,
    },
    #[serde(rename = "rule")]
    Rule {
        // 同一ルール内の複数の条件は「必ずAND」として扱う
        conditions: Vec<RuleCondition>,
    },
}

// 過去の構造体（マイグレーション用）
#[derive(Debug, Clone, Deserialize)]
struct LegacyExclusionRule {
    pub match_type: String, // "AND" or "OR"
    pub conditions: Vec<RuleCondition>,
}

pub fn parse_rules(json_str: &str) -> RuleNode {
    if json_str.trim().is_empty() {
        return RuleNode::Group { match_type: "OR".to_string(), children: vec![] };
    }

    // まずは新しいツリー構造のパースを試みる
    if let Ok(node) = serde_json::from_str::<RuleNode>(json_str) {
        return node;
    }

    // 失敗した場合は既存のリスト形式からのマイグレーションを試みる
    if let Ok(legacy_rules) = serde_json::from_str::<Vec<LegacyExclusionRule>>(json_str) {
        let mut children = Vec::new();
        for rule in legacy_rules {
            if rule.conditions.len() > 1 {
                if rule.match_type.to_uppercase() == "OR" {
                    let mut or_group_children = Vec::new();
                    for cond in rule.conditions {
                        or_group_children.push(RuleNode::Rule { conditions: vec![cond] });
                    }
                    children.push(RuleNode::Group { match_type: "OR".to_string(), children: or_group_children });
                } else {
                    children.push(RuleNode::Rule { conditions: rule.conditions });
                }
            } else if !rule.conditions.is_empty() {
                children.push(RuleNode::Rule { conditions: rule.conditions });
            }
        }
        return RuleNode::Group { match_type: "OR".to_string(), children };
    }

    // パース不能な場合は空のORグループ
    RuleNode::Group { match_type: "OR".to_string(), children: vec![] }
}

pub fn is_excluded(win: &WindowInfo, root: &RuleNode) -> bool {
    match_node(win, root)
}

fn match_node(win: &WindowInfo, node: &RuleNode) -> bool {
    match node {
        RuleNode::Group { match_type, children } => {
            if children.is_empty() { return false; }
            let is_and = match_type.to_uppercase() == "AND";
            for child in children {
                let matched = match_node(win, child);
                if is_and && !matched { return false; }
                if !is_and && matched { return true; }
            }
            is_and
        }
        RuleNode::Rule { conditions } => {
            if conditions.is_empty() { return false; }
            // ルール内の条件は常に AND
            for cond in conditions {
                let style_hex = format!("{:08X}", win.style);
                let ex_style_hex = format!("{:08X}", win.ex_style);
                
                let field_val = match cond.field.as_str() {
                    "process_name" => &win.process_name,
                    "class_name" => &win.class_name,
                    "title" => &win.title,
                    "style" => &style_hex,
                    "ex_style" => &ex_style_hex,
                    _ => "",
                };

                let matched = match cond.operator.as_str() {
                    "equals" => field_val == cond.value,
                    "contains" => field_val.contains(&cond.value),
                    "starts_with" => field_val.starts_with(&cond.value),
                    "ends_with" => field_val.ends_with(&cond.value),
                    _ => false,
                };

                if !matched { return false; } // ANDなので、一つでも外れたらルール不成立
            }
            true
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_mock_window(process_name: &str, class_name: &str, title: &str) -> WindowInfo {
        WindowInfo {
            hwnd: 0,
            title: title.to_string(),
            rect: crate::monitor::MonitorRect { left: 0, top: 0, right: 100, bottom: 100 },
            dpi: 96,
            is_maximized: false,
            is_minimized: false,
            is_fullscreen: false,
            process_name: process_name.to_string(),
            class_name: class_name.to_string(),
            style: 0,
            ex_style: 0,
        }
    }

    // ---------------------------------------------------------
    // 既存テスト
    // ---------------------------------------------------------
    #[test]
    fn test_and_group() {
        let win = create_mock_window("notepad.exe", "Notepad", "Untitled - Notepad");

        let rule = RuleNode::Group {
            match_type: "AND".to_string(),
            children: vec![
                RuleNode::Rule {
                    conditions: vec![
                        RuleCondition { field: "process_name".to_string(), operator: "equals".to_string(), value: "notepad.exe".to_string() }
                    ]
                },
                RuleNode::Rule {
                    conditions: vec![
                        RuleCondition { field: "class_name".to_string(), operator: "contains".to_string(), value: "Notepad".to_string() }
                    ]
                }
            ],
        };

        assert!(match_node(&win, &rule));
    }

    #[test]
    fn test_legacy_migration() {
        let json = r#"[
            {"match_type": "OR", "conditions": [
                {"field": "process_name", "operator": "equals", "value": "test1.exe"},
                {"field": "process_name", "operator": "equals", "value": "test2.exe"}
            ]}
        ]"#;

        let node = parse_rules(json);
        match node {
            RuleNode::Group { match_type, children } => {
                assert_eq!(match_type, "OR");
                assert_eq!(children.len(), 1);
                match &children[0] {
                    RuleNode::Group { match_type: sub_match, children: sub_children } => {
                        assert_eq!(sub_match, "OR");
                        assert_eq!(sub_children.len(), 2);
                    },
                    _ => panic!("Expected child group"),
                }
            },
            _ => panic!("Expected root group"),
        }
    }

    // ---------------------------------------------------------
    // 追加テスト: OR グループ
    // ---------------------------------------------------------
    #[test]
    fn test_or_group_matches_first() {
        let win = create_mock_window("chrome.exe", "Chrome_WidgetWin_1", "Google Chrome");
        let rule = RuleNode::Group {
            match_type: "OR".to_string(),
            children: vec![
                RuleNode::Rule {
                    conditions: vec![
                        RuleCondition { field: "process_name".to_string(), operator: "equals".to_string(), value: "chrome.exe".to_string() }
                    ]
                },
                RuleNode::Rule {
                    conditions: vec![
                        RuleCondition { field: "title".to_string(), operator: "equals".to_string(), value: "never".to_string() }
                    ]
                },
            ],
        };
        assert!(match_node(&win, &rule));
    }

    #[test]
    fn test_or_group_matches_second() {
        let win = create_mock_window("firefox.exe", "MozillaWindowClass", "Mozilla Firefox");
        let rule = RuleNode::Group {
            match_type: "OR".to_string(),
            children: vec![
                RuleNode::Rule {
                    conditions: vec![
                        RuleCondition { field: "process_name".to_string(), operator: "equals".to_string(), value: "chrome.exe".to_string() }
                    ]
                },
                RuleNode::Rule {
                    conditions: vec![
                        RuleCondition { field: "process_name".to_string(), operator: "equals".to_string(), value: "firefox.exe".to_string() }
                    ]
                },
            ],
        };
        assert!(match_node(&win, &rule));
    }

    #[test]
    fn test_or_group_no_match() {
        let win = create_mock_window("code.exe", "Chrome_WidgetWin_1", "Visual Studio Code");
        let rule = RuleNode::Group {
            match_type: "OR".to_string(),
            children: vec![
                RuleNode::Rule {
                    conditions: vec![
                        RuleCondition { field: "process_name".to_string(), operator: "equals".to_string(), value: "notepad.exe".to_string() }
                    ]
                },
                RuleNode::Rule {
                    conditions: vec![
                        RuleCondition { field: "process_name".to_string(), operator: "equals".to_string(), value: "calc.exe".to_string() }
                    ]
                },
            ],
        };
        assert!(!match_node(&win, &rule));
    }

    // ---------------------------------------------------------
    // 追加テスト: AND グループの否定ケース（一方の条件が不一致）
    // ---------------------------------------------------------
    #[test]
    fn test_and_group_partial_mismatch() {
        let win = create_mock_window("notepad.exe", "WrongClass", "Untitled - Notepad");
        let rule = RuleNode::Group {
            match_type: "AND".to_string(),
            children: vec![
                RuleNode::Rule {
                    conditions: vec![
                        RuleCondition { field: "process_name".to_string(), operator: "equals".to_string(), value: "notepad.exe".to_string() }
                    ]
                },
                RuleNode::Rule {
                    conditions: vec![
                        RuleCondition { field: "class_name".to_string(), operator: "equals".to_string(), value: "Notepad".to_string() }
                    ]
                },
            ],
        };
        // class_name が "WrongClass" なので AND 条件が外れて false
        assert!(!match_node(&win, &rule));
    }

    // ---------------------------------------------------------
    // 追加テスト: 各オペレーター
    // ---------------------------------------------------------
    #[test]
    fn test_operator_contains() {
        let win = create_mock_window("test.exe", "TestClass", "My Application Window");
        let rule = RuleNode::Rule {
            conditions: vec![
                RuleCondition { field: "title".to_string(), operator: "contains".to_string(), value: "Application".to_string() }
            ]
        };
        assert!(match_node(&win, &rule));
    }

    #[test]
    fn test_operator_starts_with() {
        let win = create_mock_window("test.exe", "TestClass", "Hello World");
        let rule = RuleNode::Rule {
            conditions: vec![
                RuleCondition { field: "title".to_string(), operator: "starts_with".to_string(), value: "Hello".to_string() }
            ]
        };
        assert!(match_node(&win, &rule));
    }

    #[test]
    fn test_operator_ends_with() {
        let win = create_mock_window("test.exe", "TestClass", "Hello World");
        let rule = RuleNode::Rule {
            conditions: vec![
                RuleCondition { field: "title".to_string(), operator: "ends_with".to_string(), value: "World".to_string() }
            ]
        };
        assert!(match_node(&win, &rule));
    }

    #[test]
    fn test_operator_starts_with_no_match() {
        let win = create_mock_window("test.exe", "TestClass", "Hello World");
        let rule = RuleNode::Rule {
            conditions: vec![
                RuleCondition { field: "title".to_string(), operator: "starts_with".to_string(), value: "World".to_string() }
            ]
        };
        assert!(!match_node(&win, &rule));
    }

    // ---------------------------------------------------------
    // 追加テスト: 空のルール・空のグループ
    // ---------------------------------------------------------
    #[test]
    fn test_empty_rule_conditions_returns_false() {
        let win = create_mock_window("test.exe", "TestClass", "Test");
        let rule = RuleNode::Rule { conditions: vec![] };
        assert!(!match_node(&win, &rule));
    }

    #[test]
    fn test_empty_group_returns_false() {
        let win = create_mock_window("test.exe", "TestClass", "Test");
        let rule = RuleNode::Group { match_type: "OR".to_string(), children: vec![] };
        assert!(!match_node(&win, &rule));
    }

    // ---------------------------------------------------------
    // 追加テスト: parse_rules 空文字
    // ---------------------------------------------------------
    #[test]
    fn test_parse_rules_empty_string() {
        let node = parse_rules("");
        match node {
            RuleNode::Group { match_type, children } => {
                assert_eq!(match_type, "OR");
                assert!(children.is_empty());
            },
            _ => panic!("Expected empty OR group"),
        }
    }

    #[test]
    fn test_parse_rules_invalid_json_returns_empty_group() {
        let node = parse_rules("not valid json {{");
        match node {
            RuleNode::Group { children, .. } => {
                assert!(children.is_empty());
            },
            _ => panic!("Expected fallback empty group"),
        }
    }

    // ---------------------------------------------------------
    // 追加テスト: 複数条件ルール（AND）
    // ---------------------------------------------------------
    #[test]
    fn test_rule_multiple_conditions_all_match() {
        let win = create_mock_window("notepad.exe", "Notepad", "Untitled - Notepad");
        let rule = RuleNode::Rule {
            conditions: vec![
                RuleCondition { field: "process_name".to_string(), operator: "equals".to_string(), value: "notepad.exe".to_string() },
                RuleCondition { field: "title".to_string(), operator: "contains".to_string(), value: "Notepad".to_string() },
            ],
        };
        assert!(match_node(&win, &rule));
    }

    #[test]
    fn test_rule_multiple_conditions_one_mismatch() {
        let win = create_mock_window("notepad.exe", "Notepad", "Different Title");
        let rule = RuleNode::Rule {
            conditions: vec![
                RuleCondition { field: "process_name".to_string(), operator: "equals".to_string(), value: "notepad.exe".to_string() },
                RuleCondition { field: "title".to_string(), operator: "contains".to_string(), value: "Notepad".to_string() },
            ],
        };
        // タイトルが一致しないため false
        assert!(!match_node(&win, &rule));
    }
}

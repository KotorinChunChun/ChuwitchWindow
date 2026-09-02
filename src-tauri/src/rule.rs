use crate::window::WindowInfo;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RuleCondition {
    /// 対象フィールド。
    /// 文字列属性に加えて、ウィンドウ状態（is_fullscreen など）も文字列として比較する。
    pub field: String,
    /// 比較演算子。wildcard / not_wildcard では `*` と `?` を使用できる。
    pub operator: String,
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
                let Some(field_val) = field_value(win, &cond.field) else {
                    // 未知のフィールドを否定演算子で真にしないため、常に不一致とする。
                    return false;
                };

                let matched = match_condition(&field_val, &cond.operator, &cond.value);

                if !matched { return false; } // ANDなので、一つでも外れたらルール不成立
            }
            true
        }
    }
}

fn field_value(win: &WindowInfo, field: &str) -> Option<String> {
    match field {
        "process_name" => Some(win.process_name.clone()),
        "class_name" => Some(win.class_name.clone()),
        "title" => Some(win.title.clone()),
        "style" => Some(format!("{:08X}", win.style)),
        "ex_style" => Some(format!("{:08X}", win.ex_style)),
        "is_fullscreen" => Some(win.is_fullscreen.to_string()),
        "is_minimized" => Some(win.is_minimized.to_string()),
        "is_maximized" => Some(win.is_maximized.to_string()),
        _ => None,
    }
}

fn match_condition(field_value: &str, operator: &str, expected: &str) -> bool {
    match operator {
        "equals" => field_value == expected,
        "not_equals" => field_value != expected,
        "contains" => field_value.contains(expected),
        "not_contains" => !field_value.contains(expected),
        "starts_with" => field_value.starts_with(expected),
        "ends_with" => field_value.ends_with(expected),
        "wildcard" => wildcard_match(field_value, expected),
        "not_wildcard" => !wildcard_match(field_value, expected),
        _ => false,
    }
}

/// Windows のファイル選択で馴染みのある glob 形式で比較する。
/// `*` は0文字以上、`?` は任意の1文字に一致し、大文字・小文字は区別しない。
fn wildcard_match(value: &str, pattern: &str) -> bool {
    let value: Vec<char> = value.to_lowercase().chars().collect();
    let pattern: Vec<char> = pattern.to_lowercase().chars().collect();
    let mut previous = vec![false; value.len() + 1];
    previous[0] = true;

    for token in pattern {
        let mut current = vec![false; value.len() + 1];
        if token == '*' {
            current[0] = previous[0];
        }

        for index in 1..=value.len() {
            current[index] = match token {
                '*' => previous[index] || current[index - 1],
                '?' => previous[index - 1],
                literal => previous[index - 1] && value[index - 1] == literal,
            };
        }
        previous = current;
    }

    previous[value.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_mock_window(process_name: &str, class_name: &str, title: &str) -> WindowInfo {
        crate::test_support::WindowBuilder::new(0)
            .title(title)
            .process(process_name)
            .class(class_name)
            .rect(0, 0, 100, 100)
            .build()
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

    #[test]
    fn test_operator_wildcard_is_case_insensitive() {
        let win = create_mock_window("MSTSC.EXE", "TscShellContainerClass", "Remote Desktop");
        let rule = RuleNode::Rule {
            conditions: vec![
                RuleCondition { field: "process_name".to_string(), operator: "wildcard".to_string(), value: "mstsc*.exe".to_string() }
            ]
        };
        assert!(match_node(&win, &rule));
    }

    #[test]
    fn test_operator_wildcard_question_mark() {
        assert!(wildcard_match("app1.exe", "app?.exe"));
        assert!(!wildcard_match("app12.exe", "app?.exe"));
    }

    #[test]
    fn test_operator_not_wildcard_supports_rdp_exception() {
        let fullscreen_rdp = WindowInfo {
            is_fullscreen: true,
            ..create_mock_window("mstsc.exe", "TscShellContainerClass", "Remote Desktop")
        };
        let fullscreen_game = WindowInfo {
            is_fullscreen: true,
            ..create_mock_window("game.exe", "GameWindow", "Game")
        };
        let rule = RuleNode::Rule {
            conditions: vec![
                RuleCondition { field: "is_fullscreen".to_string(), operator: "equals".to_string(), value: "true".to_string() },
                RuleCondition { field: "process_name".to_string(), operator: "not_wildcard".to_string(), value: "mstsc*.exe".to_string() },
            ],
        };

        assert!(!match_node(&fullscreen_rdp, &rule));
        assert!(match_node(&fullscreen_game, &rule));
    }

    #[test]
    fn test_boolean_window_state_fields() {
        let win = WindowInfo {
            is_minimized: true,
            is_maximized: false,
            ..create_mock_window("test.exe", "TestClass", "Test")
        };
        let minimized_rule = RuleNode::Rule {
            conditions: vec![RuleCondition {
                field: "is_minimized".to_string(),
                operator: "equals".to_string(),
                value: "true".to_string(),
            }],
        };
        let maximized_rule = RuleNode::Rule {
            conditions: vec![RuleCondition {
                field: "is_maximized".to_string(),
                operator: "equals".to_string(),
                value: "true".to_string(),
            }],
        };

        assert!(match_node(&win, &minimized_rule));
        assert!(!match_node(&win, &maximized_rule));
    }

    #[test]
    fn test_unknown_field_never_matches_negative_operator() {
        let win = create_mock_window("test.exe", "TestClass", "Test");
        let rule = RuleNode::Rule {
            conditions: vec![RuleCondition {
                field: "unknown".to_string(),
                operator: "not_equals".to_string(),
                value: "anything".to_string(),
            }],
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

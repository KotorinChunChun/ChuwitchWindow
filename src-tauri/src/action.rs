use crate::arrange::ArrangeType;
use tauri::{AppHandle, Emitter};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    Rotate { clockwise: bool },
    Undo,
    PinToggle,
    Escape,
    Gather,
    Arrange,
    ArrangeNow(ArrangeType),
    Swap { target: u32 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    pub message: String,
}

pub fn parse_cli_actions(args: &[String]) -> Result<Vec<Action>, ParseError> {
    let mut actions = Vec::new();
    let mut i = 1; // skip argv0
    while i < args.len() {
        let a = args[i].as_str();
        match a {
            "--startup" | "--autostart" | "--hidden" | "--hide" | "-hide" => {}
            "--launch-origin" => {
                i += 1;
                if args.get(i).is_none_or(|origin| origin.starts_with('-')) {
                    return Err(ParseError {
                        message: "--launch-origin requires a value".into(),
                    });
                }
            }
            "--rotate-cw" | "--shift-next" => actions.push(Action::Rotate { clockwise: true }),
            "--rotate-ccw" | "--shift-prev" => actions.push(Action::Rotate { clockwise: false }),
            "--undo" => actions.push(Action::Undo),
            "--pin-toggle" => actions.push(Action::PinToggle),
            "--escape" => actions.push(Action::Escape),
            "--gather" => actions.push(Action::Gather),
            "--arrange" => actions.push(Action::Arrange),
            other if other.starts_with("--arrange=") => {
                let kind = match &other[10..] {
                    "grid" => ArrangeType::Grid,
                    "vertical" => ArrangeType::Vertical,
                    "horizontal" => ArrangeType::Horizontal,
                    "cascade" => ArrangeType::Cascade,
                    _ => {
                        return Err(ParseError {
                            message: "use --arrange=grid|vertical|horizontal|cascade".into(),
                        })
                    }
                };
                actions.push(Action::ArrangeNow(kind));
            }
            "--swap" => {
                return Err(ParseError {
                    message: "use --swap=N (N = 2..9, primary vs that monitor)".into(),
                });
            }
            other if other.starts_with("--swap=") => {
                let raw = &other[7..];
                if raw.is_empty() {
                    return Err(ParseError {
                        message: "invalid --swap=".into(),
                    });
                }
                let n: u32 = raw.parse().map_err(|_| ParseError {
                    message: "invalid --swap=".into(),
                })?;
                if !(2..=9).contains(&n) {
                    return Err(ParseError {
                        message: "swap target must be 2..9".into(),
                    });
                }
                actions.push(Action::Swap { target: n });
            }
            other if other.starts_with('-') => {
                return Err(ParseError {
                    message: format!("unknown option: {other}"),
                });
            }
            _ => {}
        }
        i += 1;
    }
    Ok(actions)
}

pub fn parse_trigger_action(name: &str) -> Result<Action, ParseError> {
    match name {
        "rotate_cw" => Ok(Action::Rotate { clockwise: true }),
        "rotate_ccw" => Ok(Action::Rotate { clockwise: false }),
        "undo" => Ok(Action::Undo),
        "pin_toggle" => Ok(Action::PinToggle),
        "escape" => Ok(Action::Escape),
        "gather" => Ok(Action::Gather),
        "arrange" => Ok(Action::Arrange),
        other if other.starts_with("swap_target_") => {
            let n: u32 = other
                .trim_start_matches("swap_target_")
                .parse()
                .map_err(|_| ParseError {
                    message: format!("unknown action: {other}"),
                })?;
            if !(2..=9).contains(&n) {
                return Err(ParseError {
                    message: "swap target must be 2..9".into(),
                });
            }
            Ok(Action::Swap { target: n })
        }
        other => Err(ParseError {
            message: format!("unknown action: {other}"),
        }),
    }
}

pub fn dispatch_action(app: &AppHandle, action: Action) {
    match action {
        Action::Rotate { clockwise } => crate::logic::handle_rotate(app, clockwise),
        Action::Undo => crate::logic::handle_undo(app),
        Action::PinToggle => {
            crate::pin::toggle_pin();
            let _ = app.emit("pin-toggled", ());
        }
        Action::Escape => crate::logic::handle_escape(app),
        Action::Gather => crate::logic::handle_gather(app),
        Action::Arrange => crate::show_arrange_window(app),
        Action::ArrangeNow(kind) => crate::arrange::handle_arrange_on_active_monitor(app, kind),
        Action::Swap { target } => crate::logic::handle_swap_target(app, target),
    }
}

/// アクションを1件でも実行したら true。パース失敗は Err。
pub fn dispatch_cli_action(app: &AppHandle, args: &[String]) -> Result<bool, ParseError> {
    let actions = parse_cli_actions(args)?;
    if actions.is_empty() {
        return Ok(false);
    }
    for action in actions {
        dispatch_action(app, action);
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(rest: &[&str]) -> Vec<String> {
        std::iter::once("exe".to_string())
            .chain(rest.iter().map(|s| s.to_string()))
            .collect()
    }

    #[test]
    fn shift_next_alias() {
        assert_eq!(
            parse_cli_actions(&args(&["--shift-next"])).unwrap(),
            vec![Action::Rotate { clockwise: true }]
        );
    }

    #[test]
    fn shift_prev_alias() {
        assert_eq!(
            parse_cli_actions(&args(&["--shift-prev"])).unwrap(),
            vec![Action::Rotate { clockwise: false }]
        );
    }

    #[test]
    fn rotate_cw() {
        assert_eq!(
            parse_cli_actions(&args(&["--rotate-cw"])).unwrap(),
            vec![Action::Rotate { clockwise: true }]
        );
    }

    #[test]
    fn flags_only_are_empty() {
        assert_eq!(
            parse_cli_actions(&args(&["--autostart", "--hide"])).unwrap(),
            vec![]
        );
    }

    #[test]
    fn multiple_actions_preserve_order() {
        assert_eq!(
            parse_cli_actions(&args(&["--rotate-cw", "--undo"])).unwrap(),
            vec![Action::Rotate { clockwise: true }, Action::Undo]
        );
    }

    #[test]
    fn swap_2_and_9_ok() {
        assert_eq!(
            parse_cli_actions(&args(&["--swap=2"])).unwrap(),
            vec![Action::Swap { target: 2 }]
        );
        assert_eq!(
            parse_cli_actions(&args(&["--swap=9"])).unwrap(),
            vec![Action::Swap { target: 9 }]
        );
    }

    #[test]
    fn rejects_swap_two_positional() {
        assert!(parse_cli_actions(&args(&["--swap", "1", "2"])).is_err());
    }

    #[test]
    fn cli_accepts_all_arrange_modes_and_rejects_unknown_modes() {
        assert_eq!(
            parse_cli_actions(&args(&[
                "--arrange",
                "--arrange=grid",
                "--arrange=vertical",
                "--arrange=horizontal",
                "--arrange=cascade",
            ]))
            .unwrap(),
            vec![
                Action::Arrange,
                Action::ArrangeNow(ArrangeType::Grid),
                Action::ArrangeNow(ArrangeType::Vertical),
                Action::ArrangeNow(ArrangeType::Horizontal),
                Action::ArrangeNow(ArrangeType::Cascade),
            ]
        );
        assert!(parse_cli_actions(&args(&["--arrange=other"])).is_err());
    }

    #[test]
    fn rejects_swap_empty_and_bounds() {
        assert!(parse_cli_actions(&args(&["--swap="])).is_err());
        assert!(parse_cli_actions(&args(&["--swap=0"])).is_err());
        assert!(parse_cli_actions(&args(&["--swap=1"])).is_err());
        assert!(parse_cli_actions(&args(&["--swap=10"])).is_err());
        assert!(parse_cli_actions(&args(&["--swap=abc"])).is_err());
    }

    #[test]
    fn unknown_option() {
        assert!(parse_cli_actions(&args(&["--not-a-flag"])).is_err());
    }

    #[test]
    fn trigger_all_named_actions() {
        assert_eq!(
            parse_trigger_action("pin_toggle").unwrap(),
            Action::PinToggle
        );
        assert_eq!(parse_trigger_action("escape").unwrap(), Action::Escape);
        assert_eq!(parse_trigger_action("gather").unwrap(), Action::Gather);
        assert_eq!(parse_trigger_action("arrange").unwrap(), Action::Arrange);
        assert_eq!(
            parse_trigger_action("swap_target_2").unwrap(),
            Action::Swap { target: 2 }
        );
        assert!(parse_trigger_action("nope").is_err());
    }
}

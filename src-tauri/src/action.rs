//! ショートカットキー・CLI 引数から実行される操作の定義
//!
//! ホットキーのディスパッチ（`hotkey.rs`）と CLI 引数のディスパッチ（`lib.rs`）で
//! 「どの操作を実行するか」の対応表を二重に持たないよう、操作をこの列挙型に集約する。

use tauri::{AppHandle, Emitter};

use crate::arrange::ArrangeType;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// 増加方向へシフト
    RotateCw,
    /// 減少方向へシフト
    RotateCcw,
    Undo,
    /// アクティブウィンドウのピン留めトグル (SPEC 2-1)
    PinToggle,
    /// 一斉退避 (SPEC 2-3)
    Escape,
    /// 一極集中 (SPEC 2-3)
    Gather,
    /// 自動整列ポップアップの表示 (SPEC 2-5)
    Arrange,
    /// ポップアップを出さずに、アクティブなモニターを指定の方法で整列する（CLI 用）
    ArrangeWith(ArrangeType),
    /// プライマリモニターと指定番号のモニターを入れ替え
    SwapTarget(u32),
}

impl Action {
    /// CLI 引数 1 つを操作に変換する (SPEC 2-4)。アクション以外の引数は `None`。
    ///
    /// 自動整列は、ワンショット実行ではポップアップを表示できないため、
    /// `--arrange=grid` のように整列方法を指定した形だけを受け付ける。
    pub fn from_cli_arg(arg: &str) -> Option<Self> {
        if let Some(name) = arg.strip_prefix("--arrange=") {
            return ArrangeType::parse(name).map(Self::ArrangeWith);
        }
        match arg {
            "--rotate-cw" => Some(Self::RotateCw),
            "--rotate-ccw" => Some(Self::RotateCcw),
            "--undo" => Some(Self::Undo),
            "--pin-toggle" => Some(Self::PinToggle),
            "--escape" => Some(Self::Escape),
            "--gather" => Some(Self::Gather),
            _ => arg
                .strip_prefix("--swap=")
                .and_then(|n| n.parse().ok())
                .map(Self::SwapTarget),
        }
    }
}

pub fn execute(app: &AppHandle, action: Action) {
    match action {
        Action::RotateCw => crate::logic::handle_rotate(app, true),
        Action::RotateCcw => crate::logic::handle_rotate(app, false),
        Action::Undo => crate::logic::handle_undo(app),
        Action::PinToggle => {
            crate::pin::toggle_pin();
            let _ = app.emit("pin-toggled", ());
        }
        Action::Escape => crate::logic::handle_escape(app),
        Action::Gather => crate::logic::handle_gather(app),
        Action::Arrange => crate::show_arrange_window(app),
        Action::ArrangeWith(arrange_type) => crate::arrange::handle_arrange(app, arrange_type, None),
        Action::SwapTarget(n) => crate::logic::handle_swap_target(app, n),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_cli_arg_actions() {
        assert_eq!(Action::from_cli_arg("--rotate-cw"), Some(Action::RotateCw));
        assert_eq!(Action::from_cli_arg("--rotate-ccw"), Some(Action::RotateCcw));
        assert_eq!(Action::from_cli_arg("--undo"), Some(Action::Undo));
        assert_eq!(Action::from_cli_arg("--pin-toggle"), Some(Action::PinToggle));
        assert_eq!(Action::from_cli_arg("--escape"), Some(Action::Escape));
        assert_eq!(Action::from_cli_arg("--gather"), Some(Action::Gather));
        assert_eq!(Action::from_cli_arg("--swap=3"), Some(Action::SwapTarget(3)));
        assert_eq!(Action::from_cli_arg("--arrange=grid"), Some(Action::ArrangeWith(ArrangeType::Grid)));
        assert_eq!(Action::from_cli_arg("--arrange=Vertical"), Some(Action::ArrangeWith(ArrangeType::Vertical)));
        assert_eq!(Action::from_cli_arg("--arrange=horizontal"), Some(Action::ArrangeWith(ArrangeType::Horizontal)));
        assert_eq!(Action::from_cli_arg("--arrange=cascade"), Some(Action::ArrangeWith(ArrangeType::Cascade)));
    }

    #[test]
    fn test_from_cli_arg_non_actions() {
        // 起動オプションや実行ファイルパスはアクションではない
        assert_eq!(Action::from_cli_arg("--autostart"), None);
        assert_eq!(Action::from_cli_arg("--hide"), None);
        assert_eq!(Action::from_cli_arg("C:\\app\\chuwitchwindow.exe"), None);
        // 番号が不正な --swap は無視する
        assert_eq!(Action::from_cli_arg("--swap="), None);
        assert_eq!(Action::from_cli_arg("--swap=x"), None);
        // ポップアップ表示は CLI からは受け付けない
        assert_eq!(Action::from_cli_arg("--arrange"), None);
        assert_eq!(Action::from_cli_arg("--arrange=tile"), None);
    }
}

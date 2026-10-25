use crate::monitor::MonitorRect;
use crate::window::WindowInfo;
use std::collections::VecDeque;
use std::sync::Mutex;

/// 保持する Undo 履歴の最大段数
const MAX_UNDO_LEVELS: usize = 50;

/// Undo で復元するための、操作前のウィンドウ位置
#[derive(Debug, Clone)]
pub struct MoveAction {
    pub hwnd: isize,
    pub old_rect: MonitorRect,
    /// 操作前に最大化されていたか（復元時に最大化し直す）
    pub was_maximized: bool,
}

impl From<&WindowInfo> for MoveAction {
    fn from(win: &WindowInfo) -> Self {
        Self {
            hwnd: win.hwnd,
            old_rect: win.rect.clone(),
            was_maximized: win.is_maximized,
        }
    }
}

#[derive(Default)]
pub struct HistoryState {
    pub undo_stack: Mutex<VecDeque<Vec<MoveAction>>>,
}

impl HistoryState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&self, actions: Vec<MoveAction>) {
        if actions.is_empty() {
            return;
        }
        let mut stack = self.undo_stack.lock().unwrap();
        stack.push_back(actions);
        if stack.len() > MAX_UNDO_LEVELS {
            stack.pop_front();
        }
        tracing::debug!("History pushed: total undo levels = {}", stack.len());
    }

    /// 操作対象のウィンドウ群の現在位置を 1 段の履歴として記録する
    pub fn push_windows<'a>(&self, windows: impl IntoIterator<Item = &'a WindowInfo>) {
        self.push(windows.into_iter().map(MoveAction::from).collect());
    }

    pub fn pop(&self) -> Option<Vec<MoveAction>> {
        let mut stack = self.undo_stack.lock().unwrap();
        let res = stack.pop_back();
        if res.is_some() {
            tracing::debug!("History popped: remaining levels = {}", stack.len());
        }
        res
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn action(hwnd: isize) -> MoveAction {
        MoveAction {
            hwnd,
            old_rect: MonitorRect { left: 0, top: 0, right: 100, bottom: 100 },
            was_maximized: false,
        }
    }

    #[test]
    fn test_push_pop_is_lifo() {
        let history = HistoryState::new();
        history.push(vec![action(1)]);
        history.push(vec![action(2)]);
        assert_eq!(history.pop().unwrap()[0].hwnd, 2);
        assert_eq!(history.pop().unwrap()[0].hwnd, 1);
        assert!(history.pop().is_none());
    }

    #[test]
    fn test_empty_push_is_ignored() {
        let history = HistoryState::new();
        history.push(vec![]);
        assert!(history.pop().is_none());
    }

    #[test]
    fn test_oldest_level_is_dropped() {
        let history = HistoryState::new();
        for i in 0..(MAX_UNDO_LEVELS as isize + 1) {
            history.push(vec![action(i)]);
        }
        let mut last = None;
        while let Some(actions) = history.pop() {
            last = Some(actions[0].hwnd);
        }
        // 最初に積んだ hwnd=0 は上限超過で破棄されている
        assert_eq!(last, Some(1));
    }
}

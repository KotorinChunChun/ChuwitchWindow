use crate::monitor::MonitorRect;
use std::collections::VecDeque;
use std::sync::Mutex;

#[derive(Debug, Clone)]
pub struct MoveAction {
    pub hwnd: isize,
    pub old_rect: MonitorRect,
    pub new_rect: MonitorRect,
}

pub struct HistoryState {
    pub undo_stack: Mutex<VecDeque<Vec<MoveAction>>>,
}

impl HistoryState {
    pub fn new() -> Self {
        Self {
            undo_stack: Mutex::new(VecDeque::new()),
        }
    }

    pub fn push(&self, actions: Vec<MoveAction>) {
        if actions.is_empty() {
            return;
        }
        let mut stack = self.undo_stack.lock().unwrap();
        stack.push_back(actions);
        if stack.len() > 50 {
            stack.pop_front();
        }
        tracing::debug!("History pushed: total undo levels = {}", stack.len());
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

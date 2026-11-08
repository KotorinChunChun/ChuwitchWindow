use crate::{
    action::{dispatch_action, Action},
    config::AppConfig,
};
use global_hotkey::{
    hotkey::{Code, HotKey, Modifiers},
    GlobalHotKeyEvent, GlobalHotKeyManager,
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use tauri::AppHandle;

static IS_RECORDING: AtomicBool = AtomicBool::new(false);

struct ManagerWrapper(GlobalHotKeyManager);
unsafe impl Send for ManagerWrapper {}
unsafe impl Sync for ManagerWrapper {}

static APP_HANDLE: OnceLock<AppHandle> = OnceLock::new();
static MANAGER: OnceLock<Mutex<ManagerWrapper>> = OnceLock::new();
static CURRENT_KEYS: OnceLock<Mutex<Vec<HotKey>>> = OnceLock::new();

pub fn init(app: AppHandle) {
    if APP_HANDLE.set(app).is_err() {
        return;
    }

    let manager = GlobalHotKeyManager::new().unwrap();
    let _ = MANAGER.set(Mutex::new(ManagerWrapper(manager)));
    let _ = CURRENT_KEYS.set(Mutex::new(Vec::new()));

    reload_hotkeys();

    let receiver = GlobalHotKeyEvent::receiver();

    std::thread::spawn(move || {
        while let Ok(event) = receiver.recv() {
            if let Some(app_handle) = APP_HANDLE.get() {
                if let Some(action) = action_for_event(
                    event,
                    IS_RECORDING.load(Ordering::SeqCst),
                    crate::config::load_config,
                ) {
                    dispatch_action(app_handle, action);
                }
            }
        }
    });
}

pub fn reload_hotkeys() {
    let config = crate::config::load_config();
    let keys_to_register = bindings(&config).map(|(key, _)| key);

    if let (Some(manager_lock), Some(keys_lock)) = (MANAGER.get(), CURRENT_KEYS.get()) {
        let manager_wrapper = manager_lock.lock().unwrap();
        let manager = &manager_wrapper.0;
        let mut current = keys_lock.lock().unwrap();

        // Unregister old ones if they exist
        if !current.is_empty() {
            // we have to convert to slice or array for unregister_all, or just loop
            for k in current.iter() {
                let _ = manager.unregister(*k);
            }
        }
        current.clear();

        // Register new ones
        for k in keys_to_register {
            if manager.register(k).is_ok() {
                current.push(k);
            }
        }
    }
}

fn bindings(config: &AppConfig) -> impl Iterator<Item = (HotKey, Action)> + '_ {
    [
        (&config.rotate_cw_hotkey, Action::Rotate { clockwise: true }),
        (
            &config.rotate_ccw_hotkey,
            Action::Rotate { clockwise: false },
        ),
        (&config.undo_hotkey, Action::Undo),
        (&config.pin_hotkey, Action::PinToggle),
        (&config.escape_hotkey, Action::Escape),
        (&config.gather_hotkey, Action::Gather),
        (&config.arrange_hotkey, Action::Arrange),
    ]
    .into_iter()
    .filter_map(|(chord, action)| Some((parse_hotkey(chord)?, action)))
    .chain((2..=9).filter_map(|target| {
        if config.swap_target_modifiers.is_empty() {
            return None;
        }
        let chord = format!("{}+{target}", config.swap_target_modifiers);
        Some((parse_hotkey(&chord)?, Action::Swap { target }))
    }))
}

fn action_for_event(
    event: GlobalHotKeyEvent,
    recording: bool,
    load_config: impl FnOnce() -> AppConfig,
) -> Option<Action> {
    if recording || event.state != global_hotkey::HotKeyState::Pressed {
        return None;
    }
    bindings(&load_config()).find_map(|(key, action)| (key.id() == event.id).then_some(action))
}

fn parse_hotkey(hotkey_str: &str) -> Option<HotKey> {
    if hotkey_str.is_empty() {
        return None;
    }

    let parts: Vec<&str> = hotkey_str.split('+').collect();
    let mut modifiers = Modifiers::empty();
    let mut code_string = String::new();

    for part in parts {
        let p = part.trim().to_uppercase();
        match p.as_str() {
            "SUPER" | "WIN" | "CMD" | "META" => modifiers.insert(Modifiers::SUPER),
            "CONTROL" | "CTRL" => modifiers.insert(Modifiers::CONTROL),
            "ALT" | "OPTION" => modifiers.insert(Modifiers::ALT),
            "SHIFT" => modifiers.insert(Modifiers::SHIFT),
            other => {
                // Remove prefix if mistakenly recorded
                let mut base = other.to_string();
                if base.starts_with("DIGIT") {
                    base = base.replace("DIGIT", "");
                } else if base.starts_with("KEY") {
                    base = base.replace("KEY", "");
                }
                code_string = base;
            }
        }
    }

    let code = match code_string.as_str() {
        "ARROWRIGHT" | "RIGHT" => Code::ArrowRight,
        "ARROWLEFT" | "LEFT" => Code::ArrowLeft,
        "ARROWUP" | "UP" => Code::ArrowUp,
        "ARROWDOWN" | "DOWN" => Code::ArrowDown,
        "1" => Code::Digit1,
        "2" => Code::Digit2,
        "3" => Code::Digit3,
        "4" => Code::Digit4,
        "5" => Code::Digit5,
        "6" => Code::Digit6,
        "7" => Code::Digit7,
        "8" => Code::Digit8,
        "9" => Code::Digit9,
        "0" => Code::Digit0,
        "SPACE" => Code::Space,
        "ENTER" => Code::Enter,
        "ESCAPE" | "ESC" => Code::Escape,
        s if s.len() == 1 => {
            let c = s.chars().next().unwrap();
            match c {
                'A' => Code::KeyA,
                'B' => Code::KeyB,
                'C' => Code::KeyC,
                'D' => Code::KeyD,
                'E' => Code::KeyE,
                'F' => Code::KeyF,
                'G' => Code::KeyG,
                'H' => Code::KeyH,
                'I' => Code::KeyI,
                'J' => Code::KeyJ,
                'K' => Code::KeyK,
                'L' => Code::KeyL,
                'M' => Code::KeyM,
                'N' => Code::KeyN,
                'O' => Code::KeyO,
                'P' => Code::KeyP,
                'Q' => Code::KeyQ,
                'R' => Code::KeyR,
                'S' => Code::KeyS,
                'T' => Code::KeyT,
                'U' => Code::KeyU,
                'V' => Code::KeyV,
                'W' => Code::KeyW,
                'X' => Code::KeyX,
                'Y' => Code::KeyY,
                'Z' => Code::KeyZ,
                _ => return None,
            }
        }
        _ => return None,
    };

    Some(HotKey::new(
        if modifiers.is_empty() {
            None
        } else {
            Some(modifiers)
        },
        code,
    ))
}

#[tauri::command]
pub fn check_hotkey_conflict(hotkey_str: String) -> bool {
    // Attempt to register it temporarily via global_hotkey
    if let Some(hk) = parse_hotkey(&hotkey_str) {
        // もしアプリ自身がすでに登録しているホットキーの場合は、競合とみなさない
        if let Some(keys_lock) = CURRENT_KEYS.get() {
            if let Ok(current) = keys_lock.lock() {
                if current.iter().any(|k| k.id() == hk.id()) {
                    return false;
                }
            }
        }

        if let Ok(manager) = GlobalHotKeyManager::new() {
            if manager.register(hk).is_ok() {
                let _ = manager.unregister(hk);
                return false; // No conflict
            } else {
                return true; // Conflict
            }
        }
    }
    true // If we can't parse or instantiate manager, consider it conflicted or invalid
}

#[tauri::command]
pub fn set_recording_state(is_recording: bool) {
    IS_RECORDING.store(is_recording, Ordering::SeqCst);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_config_and_keyboard_event_aliases() {
        let modifiers = Modifiers::SUPER | Modifiers::CONTROL | Modifiers::ALT;
        let arrow = HotKey::new(Some(modifiers), Code::ArrowRight);
        assert_eq!(parse_hotkey("Win+Ctrl+Alt+Right").unwrap().id(), arrow.id());
        assert_eq!(
            parse_hotkey("Meta+Control+Alt+ArrowRight").unwrap().id(),
            arrow.id()
        );
        let number = HotKey::new(Some(modifiers), Code::Digit2);
        assert_eq!(
            parse_hotkey("Win+Ctrl+Alt+Digit2").unwrap().id(),
            number.id()
        );
        assert!(parse_hotkey("Win+Ctrl").is_none());
        assert!(check_hotkey_conflict("Win+Ctrl+unknown".into()));
    }

    #[test]
    fn isolated_key_events_dispatch_each_action_and_skip_release_and_recording() {
        use global_hotkey::HotKeyState::{Pressed, Released};
        let config = AppConfig::default();
        let load = || config.clone();
        let mut verified = 0;
        for (key, expected) in bindings(&config) {
            let pressed = GlobalHotKeyEvent {
                id: key.id(),
                state: Pressed,
            };
            let released = GlobalHotKeyEvent {
                id: key.id(),
                state: Released,
            };
            assert_eq!(action_for_event(pressed, false, load), Some(expected));
            assert_eq!(action_for_event(released, false, || unreachable!()), None);
            assert_eq!(action_for_event(pressed, true, || unreachable!()), None);
            verified += 1;
        }
        assert_eq!(verified, 15);
        let unknown = GlobalHotKeyEvent {
            id: 0,
            state: Pressed,
        };
        assert_eq!(action_for_event(unknown, false, load), None);
    }
}

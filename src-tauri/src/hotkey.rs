use global_hotkey::{
    hotkey::{Code, HotKey, Modifiers},
    GlobalHotKeyEvent, GlobalHotKeyManager,
};
use std::sync::{Mutex, OnceLock};
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, Emitter};

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
            if event.state == global_hotkey::HotKeyState::Pressed {
                if IS_RECORDING.load(Ordering::SeqCst) {
                    continue; // 録画中（設定中）はアプリのショートカットを無視
                }
                
                if let Some(app_handle) = APP_HANDLE.get() {
                    let config = crate::config::load_config();
                    
                    let cw_key = parse_hotkey(&config.rotate_cw_hotkey);
                    let ccw_key = parse_hotkey(&config.rotate_ccw_hotkey);
                    let undo_key = parse_hotkey(&config.undo_hotkey);

                    if cw_key.is_some() && event.id == cw_key.unwrap().id() {
                        crate::logic::handle_rotate(app_handle, true);
                    } else if ccw_key.is_some() && event.id == ccw_key.unwrap().id() {
                        crate::logic::handle_rotate(app_handle, false);
                    } else if undo_key.is_some() && event.id == undo_key.unwrap().id() {
                        crate::logic::handle_undo(app_handle);
                    } else {
                        // v0.2: ピン留めキーの判定
                        let pin_key = parse_hotkey(&config.pin_hotkey);
                        let escape_key = parse_hotkey(&config.escape_hotkey);
                        let gather_key = parse_hotkey(&config.gather_hotkey);
                        
                        if pin_key.is_some() && event.id == pin_key.unwrap().id() {
                            crate::pin::toggle_pin();
                            let _ = app_handle.emit("pin-toggled", ());
                        } else if escape_key.is_some() && event.id == escape_key.unwrap().id() {
                            crate::logic::handle_escape(app_handle);
                        } else if gather_key.is_some() && event.id == gather_key.unwrap().id() {
                            crate::logic::handle_gather(app_handle);
                        } else if let Some(arrange_key) = parse_hotkey(&config.arrange_hotkey) {
                            if event.id == arrange_key.id() {
                                crate::show_arrange_window(app_handle);
                            } else {
                                // Check if it matches target monitors (2~9)
                                for target_num in 2..=9 {
                                    let target_hotkey_str = format!("{}+{}", config.swap_target_modifiers, target_num);
                                    if let Some(target_key) = parse_hotkey(&target_hotkey_str) {
                                        if event.id == target_key.id() {
                                            crate::logic::handle_swap_target(app_handle, target_num);
                                            break;
                                        }
                                    }
                                }
                            }
                        } else {
                            // Check if it matches target monitors (2~9)
                            for target_num in 2..=9 {
                                let target_hotkey_str = format!("{}+{}", config.swap_target_modifiers, target_num);
                                if let Some(target_key) = parse_hotkey(&target_hotkey_str) {
                                    if event.id == target_key.id() {
                                        crate::logic::handle_swap_target(app_handle, target_num);
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    });
}

pub fn reload_hotkeys() {
    let config = crate::config::load_config();
    let mut keys_to_register = Vec::new();

    if let Some(k) = parse_hotkey(&config.rotate_cw_hotkey) { keys_to_register.push(k); }
    if let Some(k) = parse_hotkey(&config.rotate_ccw_hotkey) { keys_to_register.push(k); }
    if let Some(k) = parse_hotkey(&config.undo_hotkey) { keys_to_register.push(k); }
    // v0.2: ピン留めキーの登録
    if let Some(k) = parse_hotkey(&config.pin_hotkey) { keys_to_register.push(k); }
    // v0.2: エスケープとギャザーの登録
    if let Some(k) = parse_hotkey(&config.escape_hotkey) { keys_to_register.push(k); }
    if let Some(k) = parse_hotkey(&config.gather_hotkey) { keys_to_register.push(k); }
    if let Some(k) = parse_hotkey(&config.arrange_hotkey) { keys_to_register.push(k); }
    
    // Register 2~9 target modifiers
    if !config.swap_target_modifiers.is_empty() {
        for target_num in 2..=9 {
            let target_hotkey_str = format!("{}+{}", config.swap_target_modifiers, target_num);
            if let Some(k) = parse_hotkey(&target_hotkey_str) {
                keys_to_register.push(k);
            }
        }
    }

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

    Some(HotKey::new(if modifiers.is_empty() { None } else { Some(modifiers) }, code))
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

import type { AppConfig, WindowUIData } from "../types";

export function createWindow(partial?: Partial<WindowUIData>): WindowUIData {
  return {
    hwnd: 1,
    title: "Untitled",
    process_name: "notepad.exe",
    class_name: "Notepad",
    is_pinned: false,
    is_fullscreen: false,
    is_minimized: false,
    is_maximized: false,
    style: 0,
    ex_style: 0,
    ...partial,
  };
}

export function createConfig(partial?: Partial<AppConfig>): AppConfig {
  return {
    rotate_cw_hotkey: "Win+Ctrl+Alt+Right",
    rotate_ccw_hotkey: "Win+Ctrl+Alt+Left",
    undo_hotkey: "Win+Ctrl+Alt+Z",
    swap_target_modifiers: "Win+Ctrl+Alt",
    primary_monitor_id: null,
    swap_within_groups: false,
    monitor_order: [],
    run_on_startup: false,
    run_as_admin: false,
    monitor_groups: {},
    middle_drag_enabled: false,
    alt_drag_follow_neighbors: false,
    drag_snap_to_work_area: true,
    drag_resize_across_monitors: true,
    pin_hotkey: "Win+Ctrl+Alt+P",
    escape_hotkey: "Win+Ctrl+Alt+E",
    gather_hotkey: "Win+Ctrl+Alt+G",
    arrange_hotkey: "Win+Ctrl+Alt+A",
    rule_schema_version: 1,
    exclusion_rules: "",
    ...partial,
  };
}

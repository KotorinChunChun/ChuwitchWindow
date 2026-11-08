export type AppConfig = {
  // v0.1 既存
  rotate_cw_hotkey: string;
  rotate_ccw_hotkey: string;
  undo_hotkey: string;
  swap_target_modifiers: string;
  primary_monitor_id: string | null;
  swap_within_groups: boolean;
  monitor_order: string[];
  run_on_startup: boolean;
  run_as_admin: boolean;
  monitor_groups: Record<string, number>;
  alt_drag_enabled: boolean;
  middle_drag_enabled: boolean;
  alt_drag_follow_neighbors: boolean;
  // v0.2 新規
  pin_hotkey: string;
  escape_hotkey: string;
  gather_hotkey: string;
  arrange_hotkey: string;
  rule_schema_version: number;
  exclusion_rules: string;
};

export type MonitorInfo = {
  hmonitor: number;
  name: string;
  display_number: number;
  manufacturer: string;
  serial_number: string;
  work_area: { left: number; top: number; right: number; bottom: number };
  monitor_area: { left: number; top: number; right: number; bottom: number };
  is_primary: boolean;
};

export type WindowUIData = {
    hwnd: number;
    title: string;
    process_name: string;
    class_name: string;
    is_pinned: boolean;
    is_fullscreen: boolean;
    is_minimized: boolean;
    is_maximized: boolean;
    style: number;
    ex_style: number;
};

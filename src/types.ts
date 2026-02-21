export type AppConfig = {
    rotate_cw_hotkey: string;
    rotate_ccw_hotkey: string;
    undo_hotkey: string;
    swap_target_modifiers: string;
    primary_monitor_id: string | null;
    swap_within_groups: boolean;
    monitor_order: string[];
    ignore_fullscreen: boolean;
    run_on_startup: boolean;
    run_as_admin: boolean;
    monitor_groups: Record<string, number>;
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

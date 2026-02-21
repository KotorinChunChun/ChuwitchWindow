use serde::{Deserialize, Serialize};
use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;
use windows::Win32::Foundation::{BOOL, LPARAM, RECT};
use windows::Win32::Graphics::Gdi::{
    EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFOEXW, EnumDisplayDevicesW, DISPLAY_DEVICEW
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitorRect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl From<RECT> for MonitorRect {
    fn from(r: RECT) -> Self {
        Self {
            left: r.left,
            top: r.top,
            right: r.right,
            bottom: r.bottom,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitorInfo {
    pub hmonitor: isize,
    pub name: String,
    pub display_number: u32,
    pub manufacturer: String,
    pub serial_number: String,
    pub work_area: MonitorRect,
    pub monitor_area: MonitorRect,
    pub is_primary: bool,
}

pub fn get_all_monitors() -> Vec<MonitorInfo> {
    let mut monitors = Vec::new();
    let monitors_ptr = &mut monitors as *mut Vec<MonitorInfo> as isize;

    unsafe {
        let _ = EnumDisplayMonitors(None, None, Some(monitor_enum_proc), LPARAM(monitors_ptr));
    }
    
    // Sort by position to assign display numbers (as Windows typically does)
    monitors.sort_by(|a: &MonitorInfo, b: &MonitorInfo| {
        if a.monitor_area.left != b.monitor_area.left {
            a.monitor_area.left.cmp(&b.monitor_area.left)
        } else {
            a.monitor_area.top.cmp(&b.monitor_area.top)
        }
    });
    
    for (i, m) in monitors.iter_mut().enumerate() {
        m.display_number = (i + 1) as u32;
    }

    // Try to get more details via EnumDisplayDevicesW
    enhance_monitor_details(&mut monitors);

    monitors
}

fn enhance_monitor_details(monitors: &mut [MonitorInfo]) {
    unsafe {
        for m in monitors.iter_mut() {
            // `m.name` holds the adapter name like "\\.\DISPLAY1"
            // We need to pass it as wide string to EnumDisplayDevicesW
            let adapter_name: Vec<u16> = m.name.encode_utf16().chain(std::iter::once(0)).collect();
            
            // Get the monitor attached to this adapter
            let mut monitor_device = DISPLAY_DEVICEW::default();
            monitor_device.cb = std::mem::size_of::<DISPLAY_DEVICEW>() as u32;
            
            use windows::core::PCWSTR;
            if EnumDisplayDevicesW(PCWSTR(adapter_name.as_ptr()), 0, &mut monitor_device, 0).as_bool() {
                // Parse DeviceString (usually manufacturer or generic name)
                let name_len = monitor_device.DeviceString.iter().position(|&c| c == 0).unwrap_or(monitor_device.DeviceString.len());
                let device_string = OsString::from_wide(&monitor_device.DeviceString[..name_len]).to_string_lossy().into_owned();
                m.manufacturer = device_string;
                
                // Parse DeviceID (can sometimes contain serial or specific model info)
                let id_len = monitor_device.DeviceID.iter().position(|&c| c == 0).unwrap_or(monitor_device.DeviceID.len());
                let device_id = OsString::from_wide(&monitor_device.DeviceID[..id_len]).to_string_lossy().into_owned();
                
                // For a true Serial Number we would need to read EDID from WMI or SetupAPI.
                // Here we extract the PnP ID substring from DeviceID as a simplified "Serial / Device ID" representation.
                let parts: Vec<&str> = device_id.split('\\').collect();
                if parts.len() >= 3 {
                    m.serial_number = format!("{}_{}", parts[1], parts[2]);
                } else if !device_id.is_empty() {
                    m.serial_number = device_id;
                }
            }
        }
    }
}

unsafe extern "system" fn monitor_enum_proc(
    hmonitor: HMONITOR,
    _hdc: HDC,
    _lprect: *mut RECT,
    lparam: LPARAM,
) -> BOOL {
    let monitors = &mut *(lparam.0 as *mut Vec<MonitorInfo>);
    let mut info = MONITORINFOEXW::default();
    info.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;

    if GetMonitorInfoW(hmonitor, &mut info as *mut _ as *mut _).as_bool() {
        let name_len = info.szDevice.iter().position(|&c| c == 0).unwrap_or(info.szDevice.len());
        let name = OsString::from_wide(&info.szDevice[..name_len])
            .to_string_lossy()
            .into_owned();

        let is_primary = (info.monitorInfo.dwFlags & 1u32) != 0;

        monitors.push(MonitorInfo {
            hmonitor: hmonitor.0 as isize,
            name,
            display_number: 0, // Assigned later
            manufacturer: "Generic".to_string(), // Placeholder
            serial_number: "Unknown".to_string(), // Placeholder
            work_area: info.monitorInfo.rcWork.into(),
            monitor_area: info.monitorInfo.rcMonitor.into(),
            is_primary,
        });
    }

    BOOL(1)
}

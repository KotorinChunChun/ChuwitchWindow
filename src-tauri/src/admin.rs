use windows::Win32::Foundation::{HANDLE, HWND};
use windows::Win32::Security::{GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY};
use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNOACTIVATE;
use std::ffi::OsString;
use std::os::windows::ffi::OsStrExt;

pub fn is_user_an_admin() -> bool {
    let mut handle: HANDLE = HANDLE::default();
    unsafe {
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut handle).is_ok() {
            let mut elevation = TOKEN_ELEVATION::default();
            let mut size = std::mem::size_of::<TOKEN_ELEVATION>() as u32;
            let result = GetTokenInformation(
                handle,
                TokenElevation,
                Some(&mut elevation as *mut _ as *mut _),
                size,
                &mut size,
            );
            return result.is_ok() && elevation.TokenIsElevated != 0;
        }
    }
    false
}

pub fn restart_as_admin() -> Result<(), Box<dyn std::error::Error>> {
    let exe_path = std::env::current_exe()?;
    
    let mut path_wide: Vec<u16> = OsString::from(exe_path).encode_wide().collect();
    path_wide.push(0);

    let mut verb_wide: Vec<u16> = OsString::from("runas").encode_wide().collect();
    verb_wide.push(0);

    unsafe {
        let res = ShellExecuteW(
            HWND::default(),
            windows::core::PCWSTR::from_raw(verb_wide.as_ptr()),
            windows::core::PCWSTR::from_raw(path_wide.as_ptr()),
            windows::core::PCWSTR::null(),
            windows::core::PCWSTR::null(),
            windows::Win32::UI::WindowsAndMessaging::SHOW_WINDOW_CMD(SW_SHOWNOACTIVATE.0 as i32),
        );
        
        // ShellExecute returns a value greater than 32 on success.
        if (res.0 as isize) <= 32 {
            return Err("Failed to start elevated process".into());
        }
    }

    std::process::exit(0);
}

pub fn sync_admin_startup(enable: bool) -> Result<(), Box<dyn std::error::Error>> {
    let exe_path = std::env::current_exe()?;
    let path_str = exe_path.to_string_lossy();
    let task_name = "ChuwitchWindow_AutoStart";

    if enable {
        let script = format!(
            "schtasks /Create /F /TN \"{}\" /TR \"'{}'\" /SC ONLOGON /RL HIGHEST",
            task_name, path_str
        );
        let _ = std::process::Command::new("cmd")
            .args(&["/C", &script])
            .output()?;
    } else {
        let script = format!("schtasks /Delete /F /TN \"{}\"", task_name);
        let _ = std::process::Command::new("cmd")
            .args(&["/C", &script])
            .output()?;
    }
    Ok(())
}

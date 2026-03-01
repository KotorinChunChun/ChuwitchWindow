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

pub fn register_to_path() -> Result<(), String> {
    let exe_path = std::env::current_exe().map_err(|e| e.to_string())?;
    let bin_dir = exe_path.parent().ok_or("Failed to get parent directory")?;
    let bin_dir_str = bin_dir.to_string_lossy();

    // PowerShell を使用して PATH を安全に編集
    // [Environment]::SetEnvironmentVariable('Path', $newPath, 'User')
    let script = format!(
        "$currentPath = [Environment]::GetEnvironmentVariable('Path', 'User'); \
         if ($currentPath -split ';' -notcontains '{}') {{ \
             $newPath = $currentPath + ';' + '{}'; \
             [Environment]::SetEnvironmentVariable('Path', $newPath, 'User'); \
             Write-Host 'Success'; \
         }} else {{ \
             Write-Host 'AlreadyExists'; \
         }}",
        bin_dir_str, bin_dir_str
    );

    let output = std::process::Command::new("powershell")
        .args(&["-Command", &script])
        .output()
        .map_err(|e| e.to_string())?;

    if output.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).to_string())
    }
}

pub fn check_path_registered() -> Result<bool, String> {
    let exe_path = std::env::current_exe().map_err(|e| e.to_string())?;
    let bin_dir = exe_path.parent().ok_or("Failed to get parent directory")?;
    let bin_dir_str = bin_dir.to_string_lossy();
    
    let script = format!(
        "$currentPath = [Environment]::GetEnvironmentVariable('Path', 'User'); \
         if ($currentPath -split ';' -contains '{}') {{ Write-Host 'true' }} else {{ Write-Host 'false' }}",
        bin_dir_str
    );

    let output = std::process::Command::new("powershell")
        .args(&["-Command", &script])
        .output()
        .map_err(|e| e.to_string())?;

    if output.status.success() {
        let res = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Ok(res == "true")
    } else {
        Err(String::from_utf8_lossy(&output.stderr).to_string())
    }
}

pub fn unregister_from_path() -> Result<(), String> {
    let exe_path = std::env::current_exe().map_err(|e| e.to_string())?;
    let bin_dir = exe_path.parent().ok_or("Failed to get parent directory")?;
    let bin_dir_str = bin_dir.to_string_lossy();
    
    let script = format!(
        "$currentPath = [Environment]::GetEnvironmentVariable('Path', 'User'); \
         $parts = $currentPath -split ';'; \
         $newParts = $parts | Where-Object {{ $_ -ne '{}' }}; \
         $newPath = $newParts -join ';'; \
         [Environment]::SetEnvironmentVariable('Path', $newPath, 'User'); \
         Write-Host 'Success'",
        bin_dir_str
    );

    let output = std::process::Command::new("powershell")
        .args(&["-Command", &script])
        .output()
        .map_err(|e| e.to_string())?;

    if output.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).to_string())
    }
}

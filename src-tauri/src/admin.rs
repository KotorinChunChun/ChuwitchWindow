use windows::Win32::Foundation::{HANDLE, HWND};
use windows::Win32::Security::{GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY};
use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNOACTIVATE;
use std::ffi::OsString;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::process::CommandExt;

const CREATE_NO_WINDOW: u32 = 0x08000000;

#[link(name = "kernel32")]
extern "system" {
    fn MultiByteToWideChar(
        CodePage: u32,
        dwFlags: u32,
        lpMultiByteStr: *const u8,
        cbMultiByte: i32,
        lpWideCharStr: *mut u16,
        cchWideChar: i32,
    ) -> i32;
}

// Windows のマルチバイト文字列（日本語環境では主に Shift-JIS / CP932）を UTF-8 String にデコードする関数
fn decode_output_to_string(bytes: &[u8]) -> String {
    if bytes.is_empty() {
        return String::new();
    }

    // まずは UTF-8 としてデコードを試みる
    if let Ok(s) = std::str::from_utf8(bytes) {
        return s.to_string();
    }

    // 失敗した場合は CP932 (Shift-JIS) としてデコードを試みる
    const CP_ACP: u32 = 0; // システムデフォルトの ANSI コードページ
    unsafe {
        let len = MultiByteToWideChar(
            CP_ACP,
            0,
            bytes.as_ptr(),
            bytes.len() as i32,
            std::ptr::null_mut(),
            0,
        );
        if len > 0 {
            let mut buf = vec![0u16; len as usize];
            let res = MultiByteToWideChar(
                CP_ACP,
                0,
                bytes.as_ptr(),
                bytes.len() as i32,
                buf.as_mut_ptr(),
                len,
            );
            if res > 0 {
                return String::from_utf16_lossy(&buf);
            }
        }
    }

    // デコードできない場合は損失付き UTF-8 デコード
    String::from_utf8_lossy(bytes).into_owned()
}

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

    // 直接 schtasks コマンドを実行するヘルパー
    let run_schtasks = |args: &[&str]| -> Result<std::process::Output, std::io::Error> {
        std::process::Command::new("schtasks")
            .args(args)
            .creation_flags(CREATE_NO_WINDOW)
            .output()
    };

    if enable {
        // 1. 既存のタスクがあれば削除（エラーは無視）
        let _ = run_schtasks(&["/Delete", "/F", "/TN", task_name]);

        // 2. 新規作成
        // /TR 引数はスペースなどの対策として二重引用符で囲んで指定する。自動起動時はGUI非表示パラメータを追加。
        let tr_val = format!("\"{}\" --autostart", path_str);
        let output = run_schtasks(&[
            "/Create",
            "/F",
            "/TN",
            task_name,
            "/TR",
            &tr_val,
            "/SC",
            "ONLOGON",
            "/RL",
            "HIGHEST",
        ])?;

        if !output.status.success() {
            let stderr = decode_output_to_string(&output.stderr);
            let stdout = decode_output_to_string(&output.stdout);
            return Err(format!(
                "schtasks /Create が失敗しました (exit={}):\nstdout: {}\nstderr: {}",
                output.status, stdout.trim(), stderr.trim()
            ).into());
        }
    } else {
        let output = run_schtasks(&["/Delete", "/F", "/TN", task_name])?;

        if !output.status.success() {
            let stderr = decode_output_to_string(&output.stderr);
            // タスクが見つからない場合のエラーを許容
            if stderr.contains("見つかりません") || stderr.contains("cannot find") || stderr.contains("not found") {
                return Ok(());
            }

            let stdout = decode_output_to_string(&output.stdout);
            return Err(format!(
                "schtasks /Delete が失敗しました (exit={}):\nstdout: {}\nstderr: {}",
                output.status, stdout.trim(), stderr.trim()
            ).into());
        }
    }
    Ok(())
}

#[derive(serde::Serialize, Clone)]
pub struct AutostartStatus {
    pub is_registered: bool,
    pub is_same_path: bool,
    pub registered_path: Option<String>,
}

pub fn check_autostart_registered() -> Result<AutostartStatus, String> {
    let exe_path = std::env::current_exe().map_err(|e| e.to_string())?;
    let path_str = exe_path.to_string_lossy().to_string();
    let task_name = "ChuwitchWindow_AutoStart";

    let output = std::process::Command::new("schtasks")
        .args(&["/Query", "/TN", task_name, "/XML"])
        .creation_flags(CREATE_NO_WINDOW)
        .output();

    let default_fail = AutostartStatus {
        is_registered: false,
        is_same_path: false,
        registered_path: None,
    };

    match output {
        Ok(out) => {
            if !out.status.success() {
                // Task does not exist or access denied
                return Ok(default_fail);
            }
            // Use PowerShell to get the specific executable path registered
            let script = format!(
                "[Console]::OutputEncoding = [System.Text.Encoding]::UTF8; \
                 $task = Get-ScheduledTask -TaskName '{}' -ErrorAction SilentlyContinue; \
                 if ($task) {{ Write-Output $task.Actions[0].Execute }} else {{ Write-Output 'NONE' }}",
                task_name
            );
            
            let ps_output = std::process::Command::new("powershell")
                .args(&["-NoProfile", "-Command", &script])
                .creation_flags(CREATE_NO_WINDOW)
                .output()
                .map_err(|e| e.to_string())?;
                
            if ps_output.status.success() {
                let mut res = String::from_utf8_lossy(&ps_output.stdout).trim().to_string();
                if res == "NONE" || res.is_empty() {
                    return Ok(default_fail);
                }
                // Executable path might have quotes, remove them
                res = res.replace("\"", "").replace("'", "");
                
                let is_same = res.eq_ignore_ascii_case(&path_str);
                
                return Ok(AutostartStatus {
                    is_registered: true,
                    is_same_path: is_same,
                    registered_path: Some(res),
                });
            }

            Ok(default_fail)
        }
        Err(_) => Ok(default_fail),
    }
}

pub fn register_to_path() -> Result<(), String> {
    let exe_path = std::env::current_exe().map_err(|e| e.to_string())?;
    let bin_dir = exe_path.parent().ok_or("Failed to get parent directory")?;
    let bin_dir_str = bin_dir.to_string_lossy();

    let script = format!(
        "[Console]::OutputEncoding = [System.Text.Encoding]::UTF8; \
         $currentPath = [Environment]::GetEnvironmentVariable('Path', 'User'); \
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
        .args(&["-NoProfile", "-Command", &script])
        .creation_flags(CREATE_NO_WINDOW)
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
        "[Console]::OutputEncoding = [System.Text.Encoding]::UTF8; \
         $currentPath = [Environment]::GetEnvironmentVariable('Path', 'User'); \
         if ($currentPath -split ';' -contains '{}') {{ Write-Host 'true' }} else {{ Write-Host 'false' }}",
        bin_dir_str
    );

    let output = std::process::Command::new("powershell")
        .args(&["-NoProfile", "-Command", &script])
        .creation_flags(CREATE_NO_WINDOW)
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
        "[Console]::OutputEncoding = [System.Text.Encoding]::UTF8; \
         $currentPath = [Environment]::GetEnvironmentVariable('Path', 'User'); \
         $parts = $currentPath -split ';'; \
         $newParts = $parts | Where-Object {{ $_ -ne '{}' }}; \
         $newPath = $newParts -join ';'; \
         [Environment]::SetEnvironmentVariable('Path', $newPath, 'User'); \
         Write-Host 'Success'",
        bin_dir_str
    );

    let output = std::process::Command::new("powershell")
        .args(&["-NoProfile", "-Command", &script])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| e.to_string())?;

    if output.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).to_string())
    }
}

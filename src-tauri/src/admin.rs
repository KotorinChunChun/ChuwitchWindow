//! 管理者権限・スタートアップ登録・PATH 登録など、Windows シェルとの連携処理

use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{ERROR_FILE_NOT_FOUND, HANDLE, HWND, LPARAM, WPARAM};
use windows::Win32::Security::{GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY};
use windows::Win32::System::Registry::{
    RegCloseKey, RegGetValueW, RegOpenKeyExW, RegSetValueExW, HKEY, HKEY_CURRENT_USER,
    KEY_SET_VALUE, REG_EXPAND_SZ, REG_VALUE_TYPE, RRF_NOEXPAND, RRF_RT_REG_EXPAND_SZ, RRF_RT_REG_SZ,
};
use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::{
    SendMessageTimeoutW, HWND_BROADCAST, SHOW_WINDOW_CMD, SMTO_ABORTIFHUNG, SW_SHOWNOACTIVATE,
    SW_SHOWNORMAL, WM_SETTINGCHANGE,
};
use std::os::windows::process::CommandExt;

const CREATE_NO_WINDOW: u32 = 0x08000000;

/// スタートアップ用にタスクスケジューラへ登録するタスク名
/// （アンインストーラー `nsis/hooks.nsh` でも同じ名前で削除している）
const AUTOSTART_TASK_NAME: &str = "ChuwitchWindow_AutoStart";

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

/// NUL 終端の UTF-16 文字列に変換する
fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// ShellExecuteW でファイルや URL を開く
fn shell_execute(verb: &str, file: &str, show: SHOW_WINDOW_CMD) -> Result<(), String> {
    let verb_wide = to_wide(verb);
    let file_wide = to_wide(file);
    let res = unsafe {
        ShellExecuteW(
            HWND::default(),
            PCWSTR(verb_wide.as_ptr()),
            PCWSTR(file_wide.as_ptr()),
            PCWSTR::null(),
            PCWSTR::null(),
            show,
        )
    };
    // ShellExecute returns a value greater than 32 on success.
    if res.0 <= 32 {
        return Err(format!("ShellExecute failed (code={}): {}", res.0, file));
    }
    Ok(())
}

/// 既定のブラウザで URL を開く。
/// `cmd /C start` を経由すると `&` を含む URL が途中で切れ、コンソールも一瞬表示されるため使わない。
pub fn open_url(url: &str) -> Result<(), String> {
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err(format!("Unsupported URL: {}", url));
    }
    shell_execute("open", url, SW_SHOWNORMAL)
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
    shell_execute("runas", &exe_path.to_string_lossy(), SW_SHOWNOACTIVATE)
        .map_err(|_| "Failed to start elevated process")?;
    std::process::exit(0);
}

/// schtasks をコンソールウィンドウを出さずに実行する
fn run_schtasks(args: &[&str]) -> Result<std::process::Output, std::io::Error> {
    std::process::Command::new("schtasks")
        .args(args)
        .creation_flags(CREATE_NO_WINDOW)
        .output()
}

pub fn sync_admin_startup(enable: bool) -> Result<(), Box<dyn std::error::Error>> {
    let exe_path = std::env::current_exe()?;
    let path_str = exe_path.to_string_lossy();

    if enable {
        // 1. 既存のタスクがあれば削除（エラーは無視）
        let _ = run_schtasks(&["/Delete", "/F", "/TN", AUTOSTART_TASK_NAME]);

        // 2. 新規作成
        // /TR 引数はスペースなどの対策として二重引用符で囲んで指定する。自動起動時はGUI非表示パラメータを追加。
        let tr_val = format!("\"{}\" --autostart", path_str);
        let output = run_schtasks(&[
            "/Create",
            "/F",
            "/TN",
            AUTOSTART_TASK_NAME,
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
        let output = run_schtasks(&["/Delete", "/F", "/TN", AUTOSTART_TASK_NAME])?;

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

/// `schtasks /Query /XML` の出力から、登録されている実行ファイルのパスを取り出す
fn extract_task_command(xml: &str) -> Option<String> {
    let start = xml.find("<Command>")? + "<Command>".len();
    let end = start + xml[start..].find("</Command>")?;
    let command = xml[start..end]
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&");
    // Executable path might have quotes, remove them
    let command = command.trim().replace(['"', '\''], "");
    if command.is_empty() { None } else { Some(command) }
}

pub fn check_autostart_registered() -> Result<AutostartStatus, String> {
    let exe_path = std::env::current_exe().map_err(|e| e.to_string())?;
    let path_str = exe_path.to_string_lossy().to_string();

    let not_registered = AutostartStatus {
        is_registered: false,
        is_same_path: false,
        registered_path: None,
    };

    let out = match run_schtasks(&["/Query", "/TN", AUTOSTART_TASK_NAME, "/XML"]) {
        Ok(out) if out.status.success() => out,
        // Task does not exist or access denied
        _ => return Ok(not_registered),
    };
    let Some(registered) = extract_task_command(&decode_output_to_string(&out.stdout)) else {
        return Ok(not_registered);
    };

    Ok(AutostartStatus {
        is_registered: true,
        is_same_path: registered.eq_ignore_ascii_case(&path_str),
        registered_path: Some(registered),
    })
}

// ------------------------------------
// ユーザー環境変数 PATH への登録
// ------------------------------------

/// PATH の 1 要素とディレクトリが同じ場所を指すか（大文字小文字・前後の空白・末尾の `\` を無視）
fn is_same_dir(entry: &str, dir: &str) -> bool {
    let normalize = |s: &str| s.trim().trim_end_matches('\\').to_lowercase();
    !entry.trim().is_empty() && normalize(entry) == normalize(dir)
}

fn path_contains(path: &str, dir: &str) -> bool {
    path.split(';').any(|entry| is_same_dir(entry, dir))
}

fn path_with_entry(path: &str, dir: &str) -> String {
    if path_contains(path, dir) {
        return path.to_string();
    }
    let path = path.trim_end_matches(';');
    if path.is_empty() {
        dir.to_string()
    } else {
        format!("{};{}", path, dir)
    }
}

fn path_without_entry(path: &str, dir: &str) -> String {
    path.split(';')
        .filter(|entry| !is_same_dir(entry, dir))
        .collect::<Vec<_>>()
        .join(";")
}

/// HKCU 配下の文字列値を、`%USERPROFILE%` などを展開しないまま値の型ごと読み出す。値が無ければ `None`。
fn read_reg_string(subkey: PCWSTR, value_name: PCWSTR) -> Result<Option<(String, REG_VALUE_TYPE)>, String> {
    let flags = RRF_RT_REG_SZ | RRF_RT_REG_EXPAND_SZ | RRF_NOEXPAND;
    let mut value_type = REG_VALUE_TYPE::default();
    let mut size: u32 = 0;
    unsafe {
        if let Err(e) = RegGetValueW(HKEY_CURRENT_USER, subkey, value_name, flags, Some(&mut value_type), None, Some(&mut size)) {
            if e.code() == ERROR_FILE_NOT_FOUND.to_hresult() {
                return Ok(None);
            }
            return Err(e.to_string());
        }
        let mut buf = vec![0u16; (size as usize).div_ceil(2)];
        RegGetValueW(
            HKEY_CURRENT_USER,
            subkey,
            value_name,
            flags,
            Some(&mut value_type),
            Some(buf.as_mut_ptr().cast()),
            Some(&mut size),
        )
        .map_err(|e| e.to_string())?;
        let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
        Ok(Some((String::from_utf16_lossy(&buf[..len]), value_type)))
    }
}

/// HKCU 配下の既存キーへ文字列値を書き込む
fn write_reg_string(subkey: PCWSTR, value_name: PCWSTR, value: &str, value_type: REG_VALUE_TYPE) -> Result<(), String> {
    let data: Vec<u8> = to_wide(value).into_iter().flat_map(u16::to_le_bytes).collect();
    unsafe {
        let mut key = HKEY::default();
        RegOpenKeyExW(HKEY_CURRENT_USER, subkey, 0, KEY_SET_VALUE, &mut key).map_err(|e| e.to_string())?;
        let result = RegSetValueExW(key, value_name, 0, value_type, Some(&data));
        let _ = RegCloseKey(key);
        result.map_err(|e| e.to_string())
    }
}

/// ユーザー環境変数 Path（`HKCU\Environment`）を値の型ごと読み出す。
/// .NET の `[Environment]::SetEnvironmentVariable` は `%USERPROFILE%` 等を展開済みの REG_SZ で
/// 書き戻してしまうため、レジストリを直接読み書きして値と型をそのまま保つ。
fn read_user_path() -> Result<(String, REG_VALUE_TYPE), String> {
    let value = read_reg_string(w!("Environment"), w!("Path"))
        .map_err(|e| format!("ユーザー環境変数 Path の読み取りに失敗しました: {}", e))?;
    // ユーザー PATH 未定義なら新規作成扱い
    Ok(value.unwrap_or((String::new(), REG_EXPAND_SZ)))
}

fn write_user_path(value: &str, value_type: REG_VALUE_TYPE) -> Result<(), String> {
    write_reg_string(w!("Environment"), w!("Path"), value, value_type)
        .map_err(|e| format!("ユーザー環境変数 Path の書き込みに失敗しました: {}", e))?;
    // 起動中のエクスプローラー等へ環境変数の変更を通知する（新しく開くターミナルに反映される）
    unsafe {
        SendMessageTimeoutW(
            HWND_BROADCAST,
            WM_SETTINGCHANGE,
            WPARAM(0),
            LPARAM(w!("Environment").as_ptr() as isize),
            SMTO_ABORTIFHUNG,
            5000,
            None,
        );
    }
    Ok(())
}

fn exe_dir() -> Result<String, String> {
    let exe_path = std::env::current_exe().map_err(|e| e.to_string())?;
    let bin_dir = exe_path.parent().ok_or("Failed to get parent directory")?;
    Ok(bin_dir.to_string_lossy().into_owned())
}

pub fn register_to_path() -> Result<(), String> {
    let dir = exe_dir()?;
    let (path, value_type) = read_user_path()?;
    if path_contains(&path, &dir) {
        return Ok(());
    }
    write_user_path(&path_with_entry(&path, &dir), value_type)
}

pub fn check_path_registered() -> Result<bool, String> {
    let dir = exe_dir()?;
    let (path, _) = read_user_path()?;
    Ok(path_contains(&path, &dir))
}

pub fn unregister_from_path() -> Result<(), String> {
    let dir = exe_dir()?;
    let (path, value_type) = read_user_path()?;
    if !path_contains(&path, &dir) {
        return Ok(());
    }
    write_user_path(&path_without_entry(&path, &dir), value_type)
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIR: &str = r"C:\Program Files\ChuwitchWindow";

    #[test]
    fn test_path_contains_ignores_case_and_trailing_separator() {
        assert!(path_contains(r"C:\a;c:\program files\chuwitchwindow\;C:\b", DIR));
        assert!(path_contains(r" C:\Program Files\ChuwitchWindow ", DIR));
        assert!(!path_contains(r"C:\Program Files\ChuwitchWindow2;C:\b", DIR));
        assert!(!path_contains("", DIR));
    }

    #[test]
    fn test_path_with_entry_appends_once() {
        assert_eq!(path_with_entry("", DIR), DIR);
        assert_eq!(path_with_entry(r"%USERPROFILE%\bin;", DIR), format!(r"%USERPROFILE%\bin;{}", DIR));
        // 登録済みなら変更しない
        let registered = format!(r"C:\a;{}", DIR);
        assert_eq!(path_with_entry(&registered, DIR), registered);
    }

    #[test]
    fn test_path_without_entry_keeps_other_entries_unexpanded() {
        let path = format!(r"%USERPROFILE%\bin;{}\;C:\b", DIR.to_lowercase());
        assert_eq!(path_without_entry(&path, DIR), r"%USERPROFILE%\bin;C:\b");
    }

    #[test]
    fn test_path_entry_with_quote_in_dir() {
        // 旧実装（PowerShell 文字列への埋め込み）では ' を含むパスでスクリプトが壊れていた
        let dir = r"C:\Users\O'Brien\ChuwitchWindow";
        let added = path_with_entry(r"C:\a", dir);
        assert!(path_contains(&added, dir));
        assert_eq!(path_without_entry(&added, dir), r"C:\a");
    }

    #[test]
    fn test_extract_task_command() {
        let xml = r#"<?xml version="1.0" encoding="UTF-16"?>
<Task><Actions Context="Author"><Exec>
  <Command>"C:\Program Files\R&amp;D\chuwitchwindow.exe"</Command>
  <Arguments>--autostart</Arguments>
</Exec></Actions></Task>"#;
        assert_eq!(
            extract_task_command(xml).as_deref(),
            Some(r"C:\Program Files\R&D\chuwitchwindow.exe")
        );
        assert_eq!(extract_task_command("<Task></Task>"), None);
        assert_eq!(extract_task_command("<Command></Command>"), None);
    }
}

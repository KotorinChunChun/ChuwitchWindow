use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::process::CommandExt;
use std::path::Path;
use std::process::{Command, Output};
use windows::Win32::Foundation::{HANDLE, HWND};
use windows::Win32::Security::{GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY};
use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNOACTIVATE;

const CREATE_NO_WINDOW: u32 = 0x08000000;
const STARTUP_TASK: &str = "ChuwitchWindow_AutoStart";

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
            let _ = windows::Win32::Foundation::CloseHandle(handle);
            return result.is_ok() && elevation.TokenIsElevated != 0;
        }
    }
    false
}

pub fn restart_as_admin() -> Result<(), Box<dyn std::error::Error>> {
    let exe_path = std::env::current_exe()?;
    request_elevation(&exe_path, |verb, exe| unsafe {
        let verb: Vec<u16> = verb.encode_wide().chain(Some(0)).collect();
        let exe: Vec<u16> = exe.as_os_str().encode_wide().chain(Some(0)).collect();
        ShellExecuteW(
            HWND::default(),
            windows::core::PCWSTR::from_raw(verb.as_ptr()),
            windows::core::PCWSTR::from_raw(exe.as_ptr()),
            windows::core::PCWSTR::null(),
            windows::core::PCWSTR::null(),
            windows::Win32::UI::WindowsAndMessaging::SHOW_WINDOW_CMD(SW_SHOWNOACTIVATE.0 as i32),
        )
        .0
    })?;
    std::process::exit(0);
}

fn request_elevation(
    exe: &Path,
    shell: impl FnOnce(&OsStr, &Path) -> isize,
) -> Result<(), &'static str> {
    if shell(OsStr::new("runas"), exe) <= 32 {
        return Err("Failed to start elevated process");
    }
    Ok(())
}

fn run_schtasks(args: &[&str]) -> std::io::Result<Output> {
    Command::new("schtasks")
        .args(args)
        .creation_flags(CREATE_NO_WINDOW)
        .output()
}

pub fn sync_admin_startup(enable: bool) -> Result<(), Box<dyn std::error::Error>> {
    let exe_path = std::env::current_exe()?;
    sync_startup(&exe_path, enable, run_schtasks)
}

fn sync_startup(
    exe_path: &Path,
    enable: bool,
    mut run_schtasks: impl FnMut(&[&str]) -> std::io::Result<Output>,
) -> Result<(), Box<dyn std::error::Error>> {
    let path_str = exe_path.to_string_lossy();
    let task_name = STARTUP_TASK;

    if enable {
        // 1. 既存のタスクがあれば削除（エラーは無視）
        let _ = run_schtasks(&["/Delete", "/F", "/TN", task_name]);

        // 2. 新規作成
        // /TR 引数はスペースなどの対策として二重引用符で囲んで指定する。自動起動時はGUI非表示パラメータを追加。
        let tr_val = format!("\"{}\" --autostart", path_str);
        let output = run_schtasks(&[
            "/Create", "/F", "/TN", task_name, "/TR", &tr_val, "/SC", "ONLOGON", "/RL", "HIGHEST",
        ])?;

        if !output.status.success() {
            let stderr = decode_output_to_string(&output.stderr);
            let stdout = decode_output_to_string(&output.stdout);
            return Err(format!(
                "schtasks /Create が失敗しました (exit={}):\nstdout: {}\nstderr: {}",
                output.status,
                stdout.trim(),
                stderr.trim()
            )
            .into());
        }
    } else {
        let output = run_schtasks(&["/Delete", "/F", "/TN", task_name])?;

        if !output.status.success() {
            let stderr = decode_output_to_string(&output.stderr);
            // タスクが見つからない場合のエラーを許容
            if stderr.contains("見つかりません")
                || stderr.contains("cannot find")
                || stderr.contains("not found")
            {
                return Ok(());
            }

            let stdout = decode_output_to_string(&output.stdout);
            return Err(format!(
                "schtasks /Delete が失敗しました (exit={}):\nstdout: {}\nstderr: {}",
                output.status,
                stdout.trim(),
                stderr.trim()
            )
            .into());
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
    let exe = std::env::current_exe().map_err(|error| error.to_string())?;
    startup_status(&exe, run_schtasks)
}

fn startup_status(
    exe: &Path,
    run: impl FnOnce(&[&str]) -> std::io::Result<Output>,
) -> Result<AutostartStatus, String> {
    let mut status = AutostartStatus {
        is_registered: false,
        is_same_path: false,
        registered_path: None,
    };
    let Ok(output) = run(&["/Query", "/TN", STARTUP_TASK, "/XML"]) else {
        return Ok(status);
    };
    if !output.status.success() {
        return Ok(status);
    }

    let bytes = output.stdout.as_slice();
    let xml = if bytes.starts_with(&[0xff, 0xfe]) || bytes.starts_with(b"<\0") {
        let units = bytes
            .strip_prefix(&[0xff, 0xfe])
            .unwrap_or(bytes)
            .chunks_exact(2)
            .map(|unit| u16::from_le_bytes([unit[0], unit[1]]))
            .collect::<Vec<_>>();
        String::from_utf16_lossy(&units)
    } else {
        decode_output_to_string(bytes)
    };
    let document = roxmltree::Document::parse(xml.trim_start_matches('\u{feff}').trim())
        .map_err(|error| format!("スタートアップ設定のXMLを読み取れません: {error}"))?;
    let command = document
        .root_element()
        .children()
        .find(|node| node.has_tag_name("Actions"))
        .and_then(|actions| actions.children().find(|node| node.has_tag_name("Exec")))
        .and_then(|exec| exec.children().find(|node| node.has_tag_name("Command")))
        .and_then(|command| command.text())
        .map(|path| path.trim().trim_matches('"'));
    if let Some(path) = command.filter(|path| !path.is_empty()) {
        status.is_registered = true;
        status.is_same_path = path.eq_ignore_ascii_case(&exe.to_string_lossy());
        status.registered_path = Some(path.to_string());
    }
    Ok(status)
}

#[derive(Clone, Copy)]
enum PathAction {
    Register,
    Query,
    Unregister,
}

struct PathStatus {
    registered: bool,
    changed: bool,
}

fn path_command(action: PathAction) -> Result<PathStatus, String> {
    use winreg::{
        enums::{HKEY_CURRENT_USER, KEY_READ, KEY_SET_VALUE},
        RegKey,
    };
    let exe = std::env::current_exe().map_err(|error| error.to_string())?;
    let bin = exe
        .parent()
        .ok_or("実行ファイルの親ディレクトリが見つかりません")?;
    let current_user = RegKey::predef(HKEY_CURRENT_USER);
    let environment = match action {
        PathAction::Register => current_user
            .create_subkey("Environment")
            .map(|(key, _)| key),
        PathAction::Query => current_user.open_subkey_with_flags("Environment", KEY_READ),
        PathAction::Unregister => {
            current_user.open_subkey_with_flags("Environment", KEY_READ | KEY_SET_VALUE)
        }
    };
    let environment = match environment {
        Err(error)
            if error.kind() == std::io::ErrorKind::NotFound
                && !matches!(action, PathAction::Register) =>
        {
            return Ok(PathStatus {
                registered: false,
                changed: false,
            });
        }
        other => other.map_err(|error| error.to_string())?,
    };
    let result = update_path(&environment, bin, action).map_err(|error| error.to_string())?;
    if result.changed {
        use windows::Win32::Foundation::{LPARAM, WPARAM};
        use windows::Win32::UI::WindowsAndMessaging::{
            SendMessageTimeoutW, HWND_BROADCAST, SMTO_ABORTIFHUNG, WM_SETTINGCHANGE,
        };
        unsafe {
            SendMessageTimeoutW(
                HWND_BROADCAST,
                WM_SETTINGCHANGE,
                WPARAM(0),
                LPARAM(windows::core::w!("Environment").as_ptr() as isize),
                SMTO_ABORTIFHUNG,
                100,
                None,
            );
        }
    }
    Ok(result)
}

fn update_path(
    key: &winreg::RegKey,
    bin: &Path,
    action: PathAction,
) -> std::io::Result<PathStatus> {
    use winreg::enums::{REG_EXPAND_SZ, REG_SZ};
    use winreg::types::{FromRegValue, ToRegValue};
    let existing = match key.get_raw_value("Path") {
        Ok(value) if matches!(value.vtype, REG_SZ | REG_EXPAND_SZ) => Some(value),
        Ok(_) => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "PATHのレジストリ値が文字列ではありません",
            ));
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(error),
    };
    let path = existing
        .as_ref()
        .map(String::from_reg_value)
        .transpose()?
        .unwrap_or_default();
    let directory = bin.to_string_lossy();
    let folded = directory.to_lowercase();
    let matches = |entry: &str| entry.to_lowercase() == folded;
    let registered = path.split(';').any(matches);
    let updated = match action {
        PathAction::Register if !registered => Some(if path.is_empty() {
            directory.into_owned()
        } else {
            format!("{path};{directory}")
        }),
        PathAction::Unregister if registered => Some(
            path.split(';')
                .filter(|entry| !matches(entry))
                .collect::<Vec<_>>()
                .join(";"),
        ),
        _ => None,
    };
    if let Some(updated) = updated {
        let mut value = updated.to_reg_value();
        if let Some(existing) = existing {
            value.vtype = existing.vtype;
        }
        key.set_raw_value("Path", &value)?;
        let verified: String = key.get_value("Path")?;
        return Ok(PathStatus {
            registered: verified.split(';').any(matches),
            changed: true,
        });
    }
    Ok(PathStatus {
        registered,
        changed: false,
    })
}

pub fn register_to_path() -> Result<(), String> {
    let status = path_command(PathAction::Register)?;
    status
        .registered
        .then_some(())
        .ok_or_else(|| "PATHへの追加を確認できません".to_string())
}

pub fn check_path_registered() -> Result<bool, String> {
    path_command(PathAction::Query).map(|status| status.registered)
}

pub fn unregister_from_path() -> Result<(), String> {
    let status = path_command(PathAction::Unregister)?;
    (!status.registered)
        .then_some(())
        .ok_or_else(|| "PATHからの解除を確認できません".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::windows::process::ExitStatusExt;

    fn output(code: u32, stdout: Vec<u8>, stderr: &[u8]) -> Output {
        Output {
            status: std::process::ExitStatus::from_raw(code),
            stdout,
            stderr: stderr.to_vec(),
        }
    }

    #[test]
    fn startup_registration_and_removal_read_back_an_isolated_scheduler() {
        let exe = Path::new(r"C:\検証 fixture\chuwitchwindow.exe");
        let mut scheduled = None::<String>;
        let mut scheduler = |args: &[&str]| {
            assert!(args.windows(2).any(|pair| pair == ["/TN", STARTUP_TASK]));
            Ok(match args[0] {
                "/Create" => {
                    assert_eq!(&args[6..], &["/SC", "ONLOGON", "/RL", "HIGHEST"]);
                    let command = args[5]
                        .strip_prefix('"')
                        .unwrap()
                        .strip_suffix("\" --autostart")
                        .unwrap();
                    scheduled = Some(command.to_string());
                    output(0, vec![], b"")
                }
                "/Delete" => {
                    if scheduled.take().is_some() {
                        output(0, vec![], b"")
                    } else {
                        output(1, vec![], b"not found")
                    }
                }
                "/Query" => {
                    if let Some(command) = &scheduled {
                        let xml = format!(
                            r#"<Task xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task"><Actions><Exec><Command>&quot;{command}&quot;</Command><Arguments>--autostart</Arguments></Exec></Actions></Task>"#
                        );
                        let bytes = [0xfeff]
                            .into_iter()
                            .chain(xml.encode_utf16())
                            .flat_map(u16::to_le_bytes)
                            .collect();
                        output(0, bytes, b"")
                    } else {
                        output(1, vec![], b"not found")
                    }
                }
                other => panic!("不明なタスク操作 {other}"),
            })
        };
        assert!(!startup_status(exe, &mut scheduler).unwrap().is_registered);
        sync_startup(exe, true, &mut scheduler).unwrap();
        let status = startup_status(exe, &mut scheduler).unwrap();
        assert!(status.is_registered && status.is_same_path);
        assert_eq!(status.registered_path.as_deref(), exe.to_str());
        let conflict = startup_status(Path::new(r"C:\別配置.exe"), &mut scheduler).unwrap();
        assert!(conflict.is_registered && !conflict.is_same_path);
        sync_startup(exe, false, &mut scheduler).unwrap();
        assert!(!startup_status(exe, &mut scheduler).unwrap().is_registered);
        sync_startup(exe, false, &mut scheduler).unwrap();
        assert!(sync_startup(exe, true, |_| Ok(output(1, vec![], b"access denied"))).is_err());
    }

    #[test]
    fn path_registration_round_trips_in_a_separate_registry_key() {
        use winreg::{
            enums::{HKEY_CURRENT_USER, REG_EXPAND_SZ},
            types::ToRegValue,
            RegKey,
        };
        let root = RegKey::predef(HKEY_CURRENT_USER);
        let name = format!(r"Software\ChuwitchWindow.CliFixture-{}", std::process::id());
        let (key, disposition) = root.create_subkey(&name).unwrap();
        assert_eq!(disposition, winreg::enums::REG_CREATED_NEW_KEY);
        struct Cleanup<'a>(&'a RegKey, &'a str);
        impl Drop for Cleanup<'_> {
            fn drop(&mut self) {
                let _ = self.0.delete_subkey(self.1);
            }
        }
        let cleanup = Cleanup(&root, &name);
        let initial = r"%SYSTEMROOT%;C:\Other";
        let mut seed = initial.to_reg_value();
        seed.vtype = REG_EXPAND_SZ;
        key.set_raw_value("Path", &seed).unwrap();
        let bin = Path::new(r"C:\検証 ' fixture");
        assert!(
            !update_path(&key, bin, PathAction::Query)
                .unwrap()
                .registered
        );
        let added = update_path(&key, bin, PathAction::Register).unwrap();
        assert!(added.registered && added.changed);
        assert!(
            !update_path(&key, bin, PathAction::Register)
                .unwrap()
                .changed
        );
        assert!(
            update_path(&key, bin, PathAction::Query)
                .unwrap()
                .registered
        );
        let value: String = key.get_value("Path").unwrap();
        assert_eq!(value, format!("{initial};{}", bin.display()));
        assert_eq!(key.get_raw_value("Path").unwrap().vtype, REG_EXPAND_SZ);
        let removed = update_path(&key, bin, PathAction::Unregister).unwrap();
        assert!(!removed.registered && removed.changed);
        assert_eq!(key.get_raw_value("Path").unwrap(), seed);
        key.set_value("Path", &7u32).unwrap();
        assert_eq!(
            update_path(&key, bin, PathAction::Register)
                .err()
                .unwrap()
                .kind(),
            std::io::ErrorKind::InvalidData,
        );
        assert_eq!(key.get_value::<u32, _>("Path").unwrap(), 7);
        drop(key);
        drop(cleanup);
        assert!(root.open_subkey(&name).is_err());
    }

    #[test]
    fn elevation_request_uses_runas_and_recognizes_failure_without_elevating() {
        let exe = Path::new(r"C:\検証 fixture\chuwitchwindow.exe");
        for (result, accepted) in [(33, true), (32, false), (5, false)] {
            assert_eq!(
                request_elevation(exe, |verb, actual| {
                    assert_eq!(verb, OsStr::new("runas"));
                    assert_eq!(actual, exe);
                    result
                })
                .is_ok(),
                accepted
            );
        }
    }
}

//! 利用者専用パイプと期限付きI/Oを画面・管理プロトコルから分離する。
use super::MAX_REQUEST_BYTES;

use anyhow::ensure;
use std::ptr::{null, null_mut};
use std::time::{Duration, Instant};
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Security::Authorization::*;
use windows_sys::Win32::Security::*;
use windows_sys::Win32::Storage::FileSystem::*;
use windows_sys::Win32::System::Pipes::*;
use windows_sys::Win32::System::RemoteDesktop::ProcessIdToSessionId;
use windows_sys::Win32::System::Threading::*;
use windows_sys::Win32::System::IO::*;

// HttpSelectorと同じ利用者SID・セッション・I/O期限の契約を使う。
fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(Some(0)).collect()
}

struct OwnedHandle(HANDLE);
impl Drop for OwnedHandle {
    fn drop(&mut self) {
        unsafe { CloseHandle(self.0) };
    }
}
struct LocalMemory(HLOCAL);
impl Drop for LocalMemory {
    fn drop(&mut self) {
        unsafe { LocalFree(self.0) };
    }
}

pub fn current_user_sid() -> anyhow::Result<String> {
    let mut token = null_mut();
    ensure!(
        unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) } != 0,
        "利用者トークンを開けません"
    );
    let _token = OwnedHandle(token);
    let mut length = 0;
    unsafe { GetTokenInformation(token, TokenUser, null_mut(), 0, &mut length) };
    ensure!(length > 0, "利用者SIDのサイズを取得できません");
    let mut buffer = vec![0usize; (length as usize).div_ceil(size_of::<usize>())];
    ensure!(
        unsafe {
            GetTokenInformation(
                token,
                TokenUser,
                buffer.as_mut_ptr().cast(),
                length,
                &mut length,
            )
        } != 0,
        "利用者SIDを取得できません"
    );
    let user = unsafe { &*buffer.as_ptr().cast::<TOKEN_USER>() };
    let mut sid = null_mut();
    ensure!(
        unsafe { ConvertSidToStringSidW(user.User.Sid, &mut sid) } != 0,
        "利用者SIDを変換できません"
    );
    let _sid = LocalMemory(sid.cast());
    let mut count = 0;
    while unsafe { *sid.add(count) } != 0 {
        count += 1;
    }
    Ok(String::from_utf16(unsafe {
        std::slice::from_raw_parts(sid, count)
    })?)
}

pub(super) struct ManagementPipe {
    handle: OwnedHandle,
}

impl ManagementPipe {
    pub(super) fn create(id: &str) -> anyhow::Result<Self> {
        let sid = current_user_sid()?;
        let name = wide(&format!(r"\\.\pipe\KotorinToys.{sid}.{id}"));
        // 所有者を指定し、.NET CurrentUserOnlyの所有者照合とも一致させる。
        let sddl = wide(&format!("O:{sid}D:P(A;;GA;;;{sid})"));
        let mut descriptor = null_mut();
        ensure!(
            unsafe {
                ConvertStringSecurityDescriptorToSecurityDescriptorW(
                    sddl.as_ptr(),
                    SDDL_REVISION_1,
                    &mut descriptor,
                    null_mut(),
                )
            } != 0,
            "管理窓口のACLを作成できません"
        );
        let _descriptor = LocalMemory(descriptor);
        let security = SECURITY_ATTRIBUTES {
            nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: descriptor,
            bInheritHandle: 0,
        };
        let handle = unsafe {
            CreateNamedPipeW(
                name.as_ptr(),
                PIPE_ACCESS_DUPLEX | FILE_FLAG_OVERLAPPED | FILE_FLAG_FIRST_PIPE_INSTANCE,
                PIPE_TYPE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
                1,
                MAX_REQUEST_BYTES as u32,
                MAX_REQUEST_BYTES as u32,
                0,
                &security,
            )
        };
        ensure!(
            handle != INVALID_HANDLE_VALUE,
            "管理窓口を作成できません: {}",
            unsafe { GetLastError() }
        );
        Ok(Self {
            handle: OwnedHandle(handle),
        })
    }

    pub(super) fn accept(&self, timeout: Duration) -> anyhow::Result<()> {
        let mut operation = self.operation()?;
        let started = unsafe { ConnectNamedPipe(self.handle.0, &mut operation.0) };
        if started == 0 && unsafe { GetLastError() } == ERROR_PIPE_CONNECTED {
            return Ok(());
        }
        self.finish(started, &mut operation.0, timeout).map(|_| ())
    }

    pub(super) fn same_session(&self) -> anyhow::Result<bool> {
        let mut client = 0;
        let mut client_session = 0;
        let mut own_session = 0;
        ensure!(
            unsafe { GetNamedPipeClientProcessId(self.handle.0, &mut client) } != 0,
            "クライアントPIDが不明です"
        );
        ensure!(
            unsafe { ProcessIdToSessionId(client, &mut client_session) } != 0
                && unsafe { ProcessIdToSessionId(std::process::id(), &mut own_session) } != 0,
            "対話セッションが不明です"
        );
        Ok(client_session == own_session)
    }

    pub(super) fn read_line(&self, deadline: Instant) -> anyhow::Result<String> {
        let mut bytes = Vec::new();
        loop {
            let mut buffer = [0u8; 512];
            let mut operation = self.operation()?;
            let started = unsafe {
                ReadFile(
                    self.handle.0,
                    buffer.as_mut_ptr(),
                    buffer.len() as u32,
                    null_mut(),
                    &mut operation.0,
                )
            };
            let count = self.finish(
                started,
                &mut operation.0,
                deadline.saturating_duration_since(Instant::now()),
            )? as usize;
            ensure!(count != 0, "空の管理要求です");
            bytes.extend_from_slice(&buffer[..count]);
            ensure!(bytes.len() <= MAX_REQUEST_BYTES, "管理要求が長すぎます");
            if let Some(end) = bytes.iter().position(|byte| *byte == b'\n') {
                return Ok(String::from_utf8(bytes[..end].to_vec())?
                    .trim_end_matches('\r')
                    .trim_start_matches('\u{feff}')
                    .to_owned());
            }
        }
    }

    pub(super) fn write_line(&self, line: &str, deadline: Instant) -> anyhow::Result<()> {
        let bytes = format!("{line}\n").into_bytes();
        let mut written = 0;
        while written < bytes.len() {
            let mut operation = self.operation()?;
            let started = unsafe {
                WriteFile(
                    self.handle.0,
                    bytes[written..].as_ptr(),
                    (bytes.len() - written) as u32,
                    null_mut(),
                    &mut operation.0,
                )
            };
            let count = self.finish(
                started,
                &mut operation.0,
                deadline.saturating_duration_since(Instant::now()),
            )? as usize;
            ensure!(count > 0, "管理応答を書き込めません");
            written += count;
        }
        Ok(())
    }

    fn operation(&self) -> anyhow::Result<(OVERLAPPED, OwnedHandle)> {
        let event = unsafe { CreateEventW(null(), 1, 0, null()) };
        ensure!(!event.is_null(), "I/Oイベントを作成できません");
        let mut operation: OVERLAPPED = unsafe { std::mem::zeroed() };
        operation.hEvent = event;
        Ok((operation, OwnedHandle(event)))
    }

    fn finish(
        &self,
        started: i32,
        operation: &mut OVERLAPPED,
        timeout: Duration,
    ) -> anyhow::Result<u32> {
        if started == 0 {
            ensure!(
                unsafe { GetLastError() } == ERROR_IO_PENDING,
                "管理I/Oを開始できません"
            );
            if unsafe {
                WaitForSingleObject(
                    operation.hEvent,
                    timeout.as_millis().min(u32::MAX as u128) as u32,
                )
            } != WAIT_OBJECT_0
            {
                // OVERLAPPEDと参照バッファの破棄前に、キャンセル完了を回収する。
                unsafe {
                    CancelIoEx(self.handle.0, operation);
                }
                let mut ignored = 0;
                unsafe {
                    GetOverlappedResult(self.handle.0, operation, &mut ignored, 1);
                }
                anyhow::bail!("管理I/Oが時間切れです");
            }
        }
        let mut count = 0;
        ensure!(
            unsafe { GetOverlappedResult(self.handle.0, operation, &mut count, 0) } != 0,
            "管理I/Oを完了できません"
        );
        Ok(count)
    }

    pub(super) fn wait_for_client_close(&self, deadline: Instant) {
        while Instant::now() < deadline {
            if unsafe {
                PeekNamedPipe(
                    self.handle.0,
                    null_mut(),
                    0,
                    null_mut(),
                    null_mut(),
                    null_mut(),
                )
            } == 0
            {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    pub(super) fn disconnect(&self) {
        unsafe { DisconnectNamedPipe(self.handle.0) };
    }
}

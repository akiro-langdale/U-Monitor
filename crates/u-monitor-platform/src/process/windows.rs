//! Windows process/thread listing via Toolhelp32.

use std::{mem::size_of, time::SystemTime};
use u_monitor_core::{
    DataSource, DiagnosticError, Measurement, PlatformResult, ProcessId, ProcessInfo, ThreadInfo,
};
use windows::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS,
    TH32CS_SNAPTHREAD, THREADENTRY32, Thread32First, Thread32Next,
};
use windows::Win32::System::Threading::GetCurrentProcessId;

fn measured<T>(value: T) -> PlatformResult<T> {
    PlatformResult::Success(Measurement {
        value,
        source: DataSource::Measured,
        timestamp: SystemTime::now().into(),
    })
}

fn wide_to_string(buf: &[u16]) -> String {
    let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..end])
}

pub(super) fn list_processes() -> PlatformResult<Vec<ProcessInfo>> {
    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    let snapshot = match snapshot {
        Ok(handle) if handle != INVALID_HANDLE_VALUE => handle,
        Ok(_) | Err(_) => {
            return PlatformResult::Error(DiagnosticError::PlatformError(
                "CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS) failed".into(),
            ));
        }
    };

    let current = u64::from(unsafe { GetCurrentProcessId() });
    let mut entry = PROCESSENTRY32W {
        dwSize: size_of::<PROCESSENTRY32W>() as u32,
        ..Default::default()
    };
    let mut processes = Vec::new();
    let mut ok = unsafe { Process32FirstW(snapshot, &mut entry) }.is_ok();
    while ok {
        let pid = u64::from(entry.th32ProcessID);
        processes.push(ProcessInfo {
            pid,
            name: wide_to_string(&entry.szExeFile),
            path: None,
            is_current: pid == current,
        });
        ok = unsafe { Process32NextW(snapshot, &mut entry) }.is_ok();
    }
    let _ = unsafe { CloseHandle(snapshot) };
    processes.sort_by_key(|process| process.pid);
    if processes.is_empty() {
        return PlatformResult::Unavailable("No processes were reported by Toolhelp".into());
    }
    measured(processes)
}

pub(super) fn list_threads(process_id: ProcessId) -> PlatformResult<Vec<ThreadInfo>> {
    let pid32 = match u32::try_from(process_id) {
        Ok(pid) => pid,
        Err(_) => {
            return PlatformResult::Unavailable("Process id does not fit Windows DWORD".into());
        }
    };
    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) };
    let snapshot = match snapshot {
        Ok(handle) if handle != INVALID_HANDLE_VALUE => handle,
        Ok(_) | Err(_) => {
            return PlatformResult::Error(DiagnosticError::PlatformError(
                "CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD) failed".into(),
            ));
        }
    };

    let mut entry = THREADENTRY32 {
        dwSize: size_of::<THREADENTRY32>() as u32,
        ..Default::default()
    };
    let mut threads = Vec::new();
    let mut ok = unsafe { Thread32First(snapshot, &mut entry) }.is_ok();
    while ok {
        if entry.th32OwnerProcessID == pid32 {
            threads.push(ThreadInfo {
                tid: u64::from(entry.th32ThreadID),
                process_pid: process_id,
                name: None,
            });
        }
        ok = unsafe { Thread32Next(snapshot, &mut entry) }.is_ok();
    }
    let _ = unsafe { CloseHandle(snapshot) };
    threads.sort_by_key(|thread| thread.tid);
    measured(threads)
}

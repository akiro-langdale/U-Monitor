//! Process and thread enumeration. MEASURED listing only — no foreign memory open.

#[cfg(windows)]
mod windows;

#[cfg(target_os = "linux")]
mod linux;

#[cfg(target_os = "macos")]
mod macos;

use u_monitor_core::{PlatformResult, ProcessId, ProcessInfo, ThreadInfo};

pub(crate) fn list_processes() -> PlatformResult<Vec<ProcessInfo>> {
    #[cfg(windows)]
    {
        windows::list_processes()
    }
    #[cfg(target_os = "linux")]
    {
        linux::list_processes()
    }
    #[cfg(target_os = "macos")]
    {
        macos::list_processes()
    }
    #[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
    {
        PlatformResult::NotImplemented(
            "Process enumeration is implemented for Windows, Linux, and macOS".into(),
        )
    }
}

pub(crate) fn list_threads(process_id: ProcessId) -> PlatformResult<Vec<ThreadInfo>> {
    #[cfg(windows)]
    {
        windows::list_threads(process_id)
    }
    #[cfg(target_os = "linux")]
    {
        linux::list_threads(process_id)
    }
    #[cfg(target_os = "macos")]
    {
        macos::list_threads(process_id)
    }
    #[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
    {
        let _ = process_id;
        PlatformResult::NotImplemented(
            "Thread enumeration is implemented for Windows, Linux, and macOS".into(),
        )
    }
}

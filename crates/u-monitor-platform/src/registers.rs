//! Register capture for the current thread and consented other threads.
#[cfg(all(windows, target_arch = "x86_64"))]
mod windows_x64;
#[cfg(all(windows, target_arch = "x86_64"))]
mod windows_x64_thread;

#[cfg(target_os = "linux")]
mod linux;

use u_monitor_core::{PlatformResult, ProcessId, RegisterSnapshot, ThreadId};

pub(crate) fn capture_current_thread() -> PlatformResult<RegisterSnapshot> {
    #[cfg(all(windows, target_arch = "x86_64"))]
    {
        windows_x64::capture()
    }
    #[cfg(not(all(windows, target_arch = "x86_64")))]
    {
        PlatformResult::NotImplemented(
            "Current-thread register capture is implemented for Windows x86-64 in this version"
                .into(),
        )
    }
}

/// Captures registers of another thread. Caller must obtain explicit UI consent first.
pub(crate) fn capture_thread(
    process_id: ProcessId,
    thread_id: ThreadId,
) -> PlatformResult<RegisterSnapshot> {
    #[cfg(all(windows, target_arch = "x86_64"))]
    {
        windows_x64_thread::capture(process_id, thread_id)
    }
    #[cfg(target_os = "linux")]
    {
        linux::capture_thread(process_id, thread_id)
    }
    #[cfg(not(any(all(windows, target_arch = "x86_64"), target_os = "linux")))]
    {
        let _ = (process_id, thread_id);
        PlatformResult::NotImplemented(
            "Other-thread register capture is implemented for Windows x86-64 and Linux".into(),
        )
    }
}

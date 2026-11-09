//! Other-thread register capture on Windows x86-64 via GetThreadContext.

use super::windows_x64::{context_to_snapshot, measured};
use u_monitor_core::{DiagnosticError, PlatformResult, ProcessId, RegisterSnapshot, ThreadId};
use windows::Win32::Foundation::{CloseHandle, ERROR_ACCESS_DENIED, HANDLE};
use windows::Win32::System::Diagnostics::Debug::{CONTEXT, CONTEXT_FULL_AMD64, GetThreadContext};
use windows::Win32::System::Threading::{
    OpenThread, ResumeThread, SuspendThread, THREAD_GET_CONTEXT, THREAD_QUERY_INFORMATION,
    THREAD_SUSPEND_RESUME,
};

pub(super) fn capture(
    process_id: ProcessId,
    thread_id: ThreadId,
) -> PlatformResult<RegisterSnapshot> {
    let _ = process_id; // Toolhelp tid is unique; OpenThread uses tid directly.
    let tid = match u32::try_from(thread_id) {
        Ok(tid) => tid,
        Err(_) => {
            return PlatformResult::Unavailable("Thread id does not fit Windows DWORD".into());
        }
    };
    let access = THREAD_GET_CONTEXT | THREAD_QUERY_INFORMATION | THREAD_SUSPEND_RESUME;
    let handle = match unsafe { OpenThread(access, false, tid) } {
        Ok(handle) => handle,
        Err(error) if error.code() == ERROR_ACCESS_DENIED.into() => {
            return PlatformResult::PermissionRequired(format!(
                "OpenThread({tid}) denied: {error}"
            ));
        }
        Err(error) => {
            return PlatformResult::Error(DiagnosticError::PlatformError(format!(
                "OpenThread({tid}) failed: {error}"
            )));
        }
    };

    let result = capture_with_handle(handle, thread_id);
    let _ = unsafe { CloseHandle(handle) };
    result
}

fn capture_with_handle(handle: HANDLE, thread_id: ThreadId) -> PlatformResult<RegisterSnapshot> {
    let suspended = unsafe { SuspendThread(handle) };
    if suspended == u32::MAX {
        return PlatformResult::Error(DiagnosticError::PlatformError(
            "SuspendThread failed".into(),
        ));
    }
    let mut context = Box::new(CONTEXT {
        ContextFlags: CONTEXT_FULL_AMD64,
        ..Default::default()
    });
    let got = unsafe { GetThreadContext(handle, context.as_mut()) };
    let _ = unsafe { ResumeThread(handle) };
    if let Err(error) = got {
        if error.code() == ERROR_ACCESS_DENIED.into() {
            return PlatformResult::PermissionRequired(format!("GetThreadContext denied: {error}"));
        }
        return PlatformResult::Error(DiagnosticError::PlatformError(format!(
            "GetThreadContext failed: {error}"
        )));
    }
    measured(context_to_snapshot(context.as_ref(), thread_id))
}

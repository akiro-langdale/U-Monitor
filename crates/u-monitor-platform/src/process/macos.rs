//! macOS process listing via sysinfo. Thread enumeration deferred (NOT_IMPLEMENTED).

use std::time::SystemTime;
use sysinfo::{ProcessesToUpdate, System};
use u_monitor_core::{DataSource, Measurement, PlatformResult, ProcessId, ProcessInfo, ThreadInfo};

fn measured<T>(value: T) -> PlatformResult<T> {
    PlatformResult::Success(Measurement {
        value,
        source: DataSource::Measured,
        timestamp: SystemTime::now().into(),
    })
}

pub(super) fn list_processes() -> PlatformResult<Vec<ProcessInfo>> {
    let mut system = System::new();
    system.refresh_processes(ProcessesToUpdate::All, true);
    let current = std::process::id() as ProcessId;
    let mut processes = Vec::new();
    for (pid, process) in system.processes() {
        let pid = pid.as_u32() as ProcessId;
        processes.push(ProcessInfo {
            pid,
            name: process.name().to_string_lossy().into_owned(),
            path: process.exe().map(|path| path.display().to_string()),
            is_current: pid == current,
        });
    }
    processes.sort_by_key(|process| process.pid);
    if processes.is_empty() {
        return PlatformResult::Unavailable("sysinfo reported no processes".into());
    }
    measured(processes)
}

pub(super) fn list_threads(_process_id: ProcessId) -> PlatformResult<Vec<ThreadInfo>> {
    PlatformResult::NotImplemented(
        "macOS thread enumeration is not implemented in this version".into(),
    )
}

//! Linux process/thread listing via `/proc`.

use std::{fs, time::SystemTime};
use u_monitor_core::{
    DataSource, DiagnosticError, Measurement, PlatformResult, ProcessId, ProcessInfo, ThreadInfo,
};

fn measured<T>(value: T) -> PlatformResult<T> {
    PlatformResult::Success(Measurement {
        value,
        source: DataSource::Measured,
        timestamp: SystemTime::now().into(),
    })
}

fn current_pid() -> ProcessId {
    std::process::id() as ProcessId
}

fn read_comm(pid: ProcessId) -> Option<String> {
    fs::read_to_string(format!("/proc/{pid}/comm"))
        .ok()
        .map(|text| text.trim_end_matches('\n').to_owned())
}

fn read_cmdline_name(pid: ProcessId) -> Option<String> {
    let bytes = fs::read(format!("/proc/{pid}/cmdline")).ok()?;
    if bytes.is_empty() {
        return None;
    }
    let first = bytes.split(|b| *b == 0).next().unwrap_or(&bytes);
    let path = String::from_utf8_lossy(first);
    path.rsplit('/').next().map(str::to_owned)
}

pub(super) fn list_processes() -> PlatformResult<Vec<ProcessInfo>> {
    let entries = match fs::read_dir("/proc") {
        Ok(entries) => entries,
        Err(error) => {
            return PlatformResult::Error(DiagnosticError::PlatformError(format!(
                "failed to read /proc: {error}"
            )));
        }
    };
    let current = current_pid();
    let mut processes = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if !name.chars().all(|ch| ch.is_ascii_digit()) {
            continue;
        }
        let Ok(pid) = name.parse::<ProcessId>() else {
            continue;
        };
        let display = read_comm(pid)
            .or_else(|| read_cmdline_name(pid))
            .unwrap_or_else(|| format!("pid:{pid}"));
        let path = fs::read_link(format!("/proc/{pid}/exe"))
            .ok()
            .map(|path| path.display().to_string());
        processes.push(ProcessInfo {
            pid,
            name: display,
            path,
            is_current: pid == current,
        });
    }
    processes.sort_by_key(|process| process.pid);
    if processes.is_empty() {
        return PlatformResult::Unavailable("No process directories found under /proc".into());
    }
    measured(processes)
}

pub(super) fn list_threads(process_id: ProcessId) -> PlatformResult<Vec<ThreadInfo>> {
    let task_root = format!("/proc/{process_id}/task");
    let entries = match fs::read_dir(&task_root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => {
            return PlatformResult::PermissionRequired(format!(
                "cannot list threads for pid {process_id}: {error}"
            ));
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return PlatformResult::Unavailable(format!("process {process_id} is gone"));
        }
        Err(error) => {
            return PlatformResult::Error(DiagnosticError::PlatformError(format!(
                "failed to read {task_root}: {error}"
            )));
        }
    };
    let mut threads = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        let Ok(tid) = name.parse::<u64>() else {
            continue;
        };
        let thread_name = fs::read_to_string(format!("{task_root}/{tid}/comm"))
            .ok()
            .map(|text| text.trim_end_matches('\n').to_owned());
        threads.push(ThreadInfo {
            tid,
            process_pid: process_id,
            name: thread_name,
        });
    }
    threads.sort_by_key(|thread| thread.tid);
    measured(threads)
}

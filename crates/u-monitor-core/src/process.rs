//! Process and thread enumeration contracts. Listing is MEASURED; foreign inspect is separate.

/// OS process identifier stored as u64 for cross-platform ABI stability.
pub type ProcessId = u64;
/// OS thread identifier stored as u64 for cross-platform ABI stability.
pub type ThreadId = u64;

/// One OS-visible process at enumeration time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessInfo {
    pub pid: ProcessId,
    pub name: String,
    /// Absolute or OS-reported image path when available.
    pub path: Option<String>,
    pub is_current: bool,
}

/// One OS-visible thread belonging to a process.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThreadInfo {
    pub tid: ThreadId,
    pub process_pid: ProcessId,
    pub name: Option<String>,
}

/// Selected inspection subject for Memory / Registers panels.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InspectionTarget {
    pub process: ProcessInfo,
    pub thread: Option<ThreadInfo>,
}

impl InspectionTarget {
    pub fn current_process(pid: ProcessId, name: impl Into<String>) -> Self {
        Self {
            process: ProcessInfo {
                pid,
                name: name.into(),
                path: None,
                is_current: true,
            },
            thread: None,
        }
    }

    pub fn is_current_process(&self) -> bool {
        self.process.is_current
    }
}

//! Current-process virtual memory map and bounded read-only samples.
#[cfg(windows)]
mod windows;

use u_monitor_core::{MemoryBytes, MemoryReadRequest, PlatformResult, VirtualMemoryRegion};

/// Hard cap for one inspector read. Keeps UI snapshots small and avoids huge copies.
pub const MAX_MEMORY_SAMPLE_BYTES: usize = 512;

pub(crate) fn discover_regions() -> PlatformResult<Vec<VirtualMemoryRegion>> {
    #[cfg(windows)]
    {
        windows::enumerate_regions()
    }
    #[cfg(not(windows))]
    {
        PlatformResult::NotImplemented(
            "Virtual memory map is implemented for Windows in this version".into(),
        )
    }
}

pub(crate) fn read_bytes(request: MemoryReadRequest) -> PlatformResult<MemoryBytes> {
    #[cfg(windows)]
    {
        windows::read_bytes(request)
    }
    #[cfg(not(windows))]
    {
        let _ = request;
        PlatformResult::NotImplemented(
            "Current-process memory reads are implemented for Windows in this version".into(),
        )
    }
}

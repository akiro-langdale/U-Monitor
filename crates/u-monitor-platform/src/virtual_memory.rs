//! Process virtual memory map and bounded read-only samples via U-Library.

#[cfg(any(windows, target_os = "linux", target_os = "macos"))]
mod library;
#[cfg(any(windows, target_os = "linux", target_os = "macos"))]
pub use library::ProcessMemory;

use u_monitor_core::{MemoryBytes, MemoryReadRequest, PlatformResult, VirtualMemoryRegion};

#[cfg(any(windows, target_os = "linux", target_os = "macos"))]
pub use u_library_memory::process_memory::MAX_MEMORY_SAMPLE_BYTES;

/// Hard cap for one inspector read. Keeps UI snapshots small and avoids huge copies.
#[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
pub const MAX_MEMORY_SAMPLE_BYTES: usize = 512;

pub(crate) fn discover_regions() -> PlatformResult<Vec<VirtualMemoryRegion>> {
    #[cfg(any(windows, target_os = "linux", target_os = "macos"))]
    {
        library::enumerate_regions()
    }
    #[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
    {
        PlatformResult::NotImplemented(
            "Virtual memory map is implemented for Windows, Linux and macOS in this version".into(),
        )
    }
}

pub(crate) fn read_bytes(request: MemoryReadRequest) -> PlatformResult<MemoryBytes> {
    #[cfg(any(windows, target_os = "linux", target_os = "macos"))]
    {
        library::read_bytes(request)
    }
    #[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
    {
        let _ = request;
        PlatformResult::NotImplemented(
            "Current-process memory reads are implemented for Windows, Linux and macOS in this version"
                .into(),
        )
    }
}

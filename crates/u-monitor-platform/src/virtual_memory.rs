//! Current-process virtual memory map and bounded read-only samples.
#[cfg(windows)]
mod windows;

#[cfg(target_os = "linux")]
mod linux;

#[cfg(target_os = "macos")]
mod macos;

// Parsed on all targets so unit tests cover Linux maps format under Windows CI.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
mod maps_parse;

use u_monitor_core::{MemoryBytes, MemoryReadRequest, PlatformResult, VirtualMemoryRegion};

/// Hard cap for one inspector read. Keeps UI snapshots small and avoids huge copies.
pub const MAX_MEMORY_SAMPLE_BYTES: usize = 512;

pub(crate) fn discover_regions() -> PlatformResult<Vec<VirtualMemoryRegion>> {
    #[cfg(windows)]
    {
        windows::enumerate_regions()
    }
    #[cfg(target_os = "linux")]
    {
        linux::enumerate_regions()
    }
    #[cfg(target_os = "macos")]
    {
        macos::enumerate_regions()
    }
    #[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
    {
        PlatformResult::NotImplemented(
            "Virtual memory map is implemented for Windows, Linux and macOS in this version".into(),
        )
    }
}

pub(crate) fn read_bytes(request: MemoryReadRequest) -> PlatformResult<MemoryBytes> {
    #[cfg(windows)]
    {
        windows::read_bytes(request)
    }
    #[cfg(target_os = "linux")]
    {
        linux::read_bytes(request)
    }
    #[cfg(target_os = "macos")]
    {
        macos::read_bytes(request)
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

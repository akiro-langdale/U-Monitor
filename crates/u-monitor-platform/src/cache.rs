//! CPU cache topology discovery. Metadata only — never reconstructed cache lines.
#[cfg(windows)]
mod windows;

#[cfg(target_os = "linux")]
mod linux;

// Available on all hosts so Linux sysfs parsers are unit-tested under Windows CI.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
mod sysfs_parse;

use u_monitor_core::{CacheTopology, PlatformResult};

pub(crate) fn discover() -> PlatformResult<CacheTopology> {
    #[cfg(windows)]
    {
        windows::discover()
    }
    #[cfg(target_os = "linux")]
    {
        linux::discover()
    }
    #[cfg(not(any(windows, target_os = "linux")))]
    {
        PlatformResult::NotImplemented(
            "Cache topology discovery is implemented for Windows and Linux in this version".into(),
        )
    }
}

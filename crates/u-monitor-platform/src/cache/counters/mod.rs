//! Hardware cache counter sampling. Never invents hits/misses.

#[cfg(target_os = "linux")]
mod linux;

use u_monitor_core::{CacheCounters, PlatformResult};

pub(super) fn sample() -> PlatformResult<CacheCounters> {
    #[cfg(target_os = "linux")]
    {
        linux::sample()
    }
    #[cfg(windows)]
    {
        PlatformResult::Unavailable(
            "Windows user-mode PMC for cache references/misses is not exposed without a driver; values are not simulated"
                .into(),
        )
    }
    #[cfg(not(any(windows, target_os = "linux")))]
    {
        PlatformResult::NotImplemented(
            "Hardware cache counters are implemented for Linux perf_event_open in this version"
                .into(),
        )
    }
}

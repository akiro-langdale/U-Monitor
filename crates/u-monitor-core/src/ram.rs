//! System RAM statistics, not access to physical memory contents.
use crate::PlatformResult;

/// Byte counts visible to the OS; not installed DIMM capacity or a cgroup quota.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RamInfo {
    pub total_bytes: PlatformResult<u64>,
    /// Backend-reported reusable memory, which may include reclaimable caches.
    /// sysinfo may estimate it on platforms without a direct OS value.
    pub available_bytes: PlatformResult<u64>,
    /// Calculated as total - available, not a sum of process resident memory.
    pub used_bytes: PlatformResult<u64>,
}

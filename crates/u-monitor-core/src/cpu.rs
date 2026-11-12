//! CPU discovery data, independent of any OS library.
use crate::PlatformResult;
use std::num::NonZeroUsize;

/// Finite CPU utilization in percent, on the inclusive 0..=100 scale.
/// A valid zero represents an observed idle interval, never missing data.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CpuUsagePercent(f32);

impl CpuUsagePercent {
    pub fn get(self) -> f32 {
        self.0
    }
}

impl TryFrom<f32> for CpuUsagePercent {
    type Error = crate::DiagnosticError;

    fn try_from(value: f32) -> Result<Self, Self::Error> {
        if value.is_finite() && (0.0..=100.0).contains(&value) {
            Ok(Self(value))
        } else {
            Err(crate::DiagnosticError::ValidationError(format!(
                "CPU usage must be finite and within 0..=100 percent: {value}"
            )))
        }
    }
}

/// An OS-visible logical processor, not a physical core. Index and name are
/// identifiers within this sampler session, not stable hardware identifiers.
#[derive(Debug, Clone, PartialEq)]
pub struct LogicalCpuUsage {
    pub index: usize,
    pub name: String,
    pub usage: PlatformResult<CpuUsagePercent>,
    /// Current operating frequency in MHz when the OS exposes it.
    /// A backend zero is treated as missing data, never as MEASURED idle clock.
    pub current_mhz: PlatformResult<u64>,
    /// Maximum frequency reported for this logical processor, when available.
    pub max_mhz: PlatformResult<u64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CpuMetrics {
    /// Observed monotonic time between completed backend refreshes. OS counter
    /// windows may differ slightly. Not the configured worker sleep interval.
    pub sampling_interval: std::time::Duration,
    /// Backend-reported total utilization, normalized to 0..=100, not a sum.
    pub total_usage: PlatformResult<CpuUsagePercent>,
    pub logical_processors: Vec<LogicalCpuUsage>,
}

#[cfg(test)]
mod usage_tests {
    use super::*;

    #[test]
    fn percent_preserves_boundaries_and_rejects_invalid_backend_values() {
        for value in [0.0, 12.5, 100.0] {
            assert_eq!(CpuUsagePercent::try_from(value).unwrap().get(), value);
        }
        for value in [-1.0, 100.1, f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            assert!(CpuUsagePercent::try_from(value).is_err());
        }
    }
}

/// OS-reported CPU information, possibly limited by a VM or container.
/// Fields have separate outcomes so missing topology never hides a model.
/// Multiple distinct model names are retained on heterogeneous/multi-socket hosts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CpuInfo {
    pub model_names: PlatformResult<Vec<String>>,
    pub logical_processors: PlatformResult<NonZeroUsize>,
    pub physical_cores: PlatformResult<NonZeroUsize>,
}

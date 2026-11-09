//! Separate educational disk latency model. Never mixed into RAM/cache MEASURED stats.

use std::time::SystemTime;
use u_monitor_core::{DataSource, Measurement};

/// Parameters for a coarse disk access latency model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiskLatencyConfig {
    pub base_latency_ns: u64,
    pub per_kib_ns: u64,
}

impl Default for DiskLatencyConfig {
    fn default() -> Self {
        Self {
            base_latency_ns: 2_000_000, // ~2 ms seek/command overhead in the toy model
            per_kib_ns: 8_000,          // transfer component
        }
    }
}

/// One simulated disk read/write timing estimate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiskAccessResult {
    pub size_bytes: u64,
    pub latency_ns: u64,
    pub base_latency_ns: u64,
    pub transfer_latency_ns: u64,
}

/// Educational disk latency estimator. Outputs are always SIMULATED.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DiskLatencyModel {
    pub config: DiskLatencyConfig,
}

impl DiskLatencyModel {
    pub fn estimate(&self, size_bytes: u64) -> Measurement<DiskAccessResult> {
        let kib = size_bytes.div_ceil(1024);
        let transfer = self.config.per_kib_ns.saturating_mul(kib);
        let latency = self.config.base_latency_ns.saturating_add(transfer);
        Measurement {
            value: DiskAccessResult {
                size_bytes,
                latency_ns: latency,
                base_latency_ns: self.config.base_latency_ns,
                transfer_latency_ns: transfer,
            },
            source: DataSource::Simulated,
            timestamp: SystemTime::now().into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disk_model_marks_simulated_and_scales_with_size() {
        let model = DiskLatencyModel::default();
        let small = model.estimate(512);
        let large = model.estimate(16 * 1024);
        assert_eq!(small.source, DataSource::Simulated);
        assert!(large.value.latency_ns > small.value.latency_ns);
        assert_eq!(
            large.value.latency_ns,
            large.value.base_latency_ns + large.value.transfer_latency_ns
        );
    }
}

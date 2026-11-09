//! Disk I/O throughput from cumulative device counters. Rates are CALCULATED.

#[cfg(windows)]
mod windows;

#[cfg(target_os = "linux")]
mod linux;

use std::time::{Duration, Instant, SystemTime};
use u_monitor_core::{DataSource, DiskIoRates, Measurement, PlatformResult};

/// Minimum gap between counter snapshots, matching CPU sampler discipline.
pub const MINIMUM_INTERVAL: Duration = Duration::from_millis(200);

#[derive(Debug, Clone, Copy)]
struct CounterSample {
    read_bytes: u64,
    write_bytes: u64,
    at: Instant,
}

#[derive(Default)]
pub(crate) struct DiskIoSampler {
    previous: Option<CounterSample>,
}

impl DiskIoSampler {
    pub(crate) fn sample(&mut self) -> PlatformResult<DiskIoRates> {
        let now = Instant::now();
        let current = match read_cumulative() {
            Ok(totals) => CounterSample {
                read_bytes: totals.0,
                write_bytes: totals.1,
                at: now,
            },
            Err(result) => return result,
        };

        let Some(previous) = self.previous.replace(current) else {
            return PlatformResult::Unavailable(
                "Disk I/O sampler warming up; first counter snapshot retained as baseline".into(),
            );
        };

        let interval = current.at.saturating_duration_since(previous.at);
        if interval < MINIMUM_INTERVAL {
            // Keep the newer baseline so the next eligible sample uses a fresh window.
            return PlatformResult::Unavailable(format!(
                "Disk I/O sample requested before MINIMUM_INTERVAL ({MINIMUM_INTERVAL:?})"
            ));
        }

        let secs = interval.as_secs_f64();
        if secs <= 0.0 {
            return PlatformResult::Unavailable("Disk I/O sample interval was zero".into());
        }

        let read_delta = current.read_bytes.saturating_sub(previous.read_bytes);
        let write_delta = current.write_bytes.saturating_sub(previous.write_bytes);
        let timestamp = SystemTime::now().into();
        PlatformResult::Success(Measurement {
            value: DiskIoRates {
                read_bytes_per_sec: calculated(read_delta as f64 / secs, timestamp),
                write_bytes_per_sec: calculated(write_delta as f64 / secs, timestamp),
                sample_interval: calculated(interval, timestamp),
            },
            source: DataSource::Calculated,
            timestamp,
        })
    }
}

fn calculated<T>(value: T, timestamp: u_monitor_core::Timestamp) -> PlatformResult<T> {
    PlatformResult::Success(Measurement {
        value,
        source: DataSource::Calculated,
        timestamp,
    })
}

fn read_cumulative() -> Result<(u64, u64), PlatformResult<DiskIoRates>> {
    #[cfg(windows)]
    {
        windows::cumulative_bytes()
    }
    #[cfg(target_os = "linux")]
    {
        linux::cumulative_bytes()
    }
    #[cfg(not(any(windows, target_os = "linux")))]
    {
        Err(PlatformResult::NotImplemented(
            "Disk I/O counters are implemented for Windows and Linux in this version".into(),
        ))
    }
}

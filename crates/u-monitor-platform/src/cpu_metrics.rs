//! CPU utilization sampling. Windows uses kernel tick counters; other targets use sysinfo.

#[cfg(windows)]
mod windows;

use std::time::{Duration, Instant};
use u_monitor_core::{
    CpuMetrics, CpuUsagePercent, DataSource, LogicalCpuUsage, Measurement, PlatformResult,
    Timestamp,
};

/// Minimum pause between counter refreshes for this crate's CPU sampler.
pub(crate) const MINIMUM_INTERVAL: Duration = Duration::from_millis(200);

pub(crate) trait CpuSamplerState {
    fn sample_at(&mut self, now: Instant) -> PlatformResult<CpuMetrics>;
}

#[cfg(windows)]
type Backend = windows::WindowsCpuBackend;

#[cfg(not(windows))]
type Backend = SysinfoCpuBackend;

#[derive(Default)]
pub(crate) struct CpuSampler {
    backend: Backend,
}

impl CpuSampler {
    pub(crate) fn sample(&mut self) -> PlatformResult<CpuMetrics> {
        self.backend.sample_at(Instant::now())
    }
}

fn measured<T>(value: T, timestamp: Timestamp) -> PlatformResult<T> {
    PlatformResult::Success(Measurement {
        value,
        source: DataSource::Measured,
        timestamp,
    })
}

fn percent(value: f32, timestamp: Timestamp) -> PlatformResult<CpuUsagePercent> {
    match CpuUsagePercent::try_from(value) {
        Ok(value) => measured(value, timestamp),
        Err(error) => PlatformResult::Error(error),
    }
}

/// Backend zeros mean "no reading", not a halted clock. Keep them Unavailable.
pub(super) fn mhz(value: u64, timestamp: Timestamp) -> PlatformResult<u64> {
    if value == 0 {
        PlatformResult::Unavailable("CPU frequency was not reported by the OS".into())
    } else {
        measured(value, timestamp)
    }
}

pub(super) struct LogicalSample {
    pub(super) name: String,
    pub(super) usage: PlatformResult<CpuUsagePercent>,
    pub(super) current_mhz: PlatformResult<u64>,
    pub(super) max_mhz: PlatformResult<u64>,
}

fn from_report(
    total: f32,
    processors: impl IntoIterator<Item = LogicalSample>,
    sampling_interval: Duration,
    timestamp: Timestamp,
) -> PlatformResult<CpuMetrics> {
    measured(
        CpuMetrics {
            sampling_interval,
            total_usage: percent(total, timestamp),
            logical_processors: processors
                .into_iter()
                .enumerate()
                .map(|(index, sample)| LogicalCpuUsage {
                    index,
                    name: sample.name,
                    usage: sample.usage,
                    current_mhz: sample.current_mhz,
                    max_mhz: sample.max_mhz,
                })
                .collect(),
        },
        timestamp,
    )
}

#[cfg(not(windows))]
#[derive(Default)]
struct SysinfoCpuBackend {
    system: sysinfo::System,
    last_refresh: Option<Instant>,
}

#[cfg(not(windows))]
impl CpuSamplerState for SysinfoCpuBackend {
    fn sample_at(&mut self, now: Instant) -> PlatformResult<CpuMetrics> {
        use std::time::SystemTime;
        use sysinfo::CpuRefreshKind;

        if !sysinfo::IS_SUPPORTED_SYSTEM {
            return PlatformResult::Unavailable(
                "sysinfo does not support this operating system".into(),
            );
        }
        if self
            .last_refresh
            .is_some_and(|last| now.duration_since(last) < MINIMUM_INTERVAL)
        {
            return PlatformResult::Unavailable(
                "CPU sampling interval is shorter than the backend minimum".into(),
            );
        }
        self.system
            .refresh_cpu_specifics(CpuRefreshKind::nothing().with_cpu_usage().with_frequency());
        let completed = Instant::now();
        let previous = self.last_refresh.replace(completed);
        if self.system.cpus().is_empty() {
            self.last_refresh = None;
            return PlatformResult::Unavailable(
                "No logical processors were reported by the OS".into(),
            );
        }
        let Some(previous) = previous else {
            return PlatformResult::Unavailable(
                "CPU counters initialized; waiting for the next measurement".into(),
            );
        };
        let timestamp = SystemTime::now().into();
        from_report(
            self.system.global_cpu_usage(),
            self.system.cpus().iter().map(|cpu| LogicalSample {
                name: cpu.name().to_owned(),
                usage: percent(cpu.cpu_usage(), timestamp),
                current_mhz: mhz(cpu.frequency(), timestamp),
                // sysinfo exposes one frequency reading per logical CPU.
                max_mhz: PlatformResult::Unavailable(
                    "maximum CPU frequency is not exposed by this backend".into(),
                ),
            }),
            completed.duration_since(previous),
            timestamp,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::UNIX_EPOCH;

    #[test]
    fn bad_cpu_values_do_not_hide_valid_processors_or_become_zero() {
        let result = from_report(
            f32::NAN,
            [
                LogicalSample {
                    name: "cpu0".into(),
                    usage: percent(0.0, UNIX_EPOCH.into()),
                    current_mhz: mhz(3200, UNIX_EPOCH.into()),
                    max_mhz: mhz(4800, UNIX_EPOCH.into()),
                },
                LogicalSample {
                    name: "cpu1".into(),
                    usage: percent(100.0, UNIX_EPOCH.into()),
                    current_mhz: mhz(0, UNIX_EPOCH.into()),
                    max_mhz: mhz(0, UNIX_EPOCH.into()),
                },
                LogicalSample {
                    name: "cpu2".into(),
                    usage: percent(-1.0, UNIX_EPOCH.into()),
                    current_mhz: mhz(1000, UNIX_EPOCH.into()),
                    max_mhz: mhz(2000, UNIX_EPOCH.into()),
                },
            ],
            Duration::from_secs(1),
            UNIX_EPOCH.into(),
        );
        let PlatformResult::Success(report) = result else {
            panic!("expected partial report")
        };
        assert_eq!(report.source, DataSource::Measured);
        assert_eq!(report.timestamp, UNIX_EPOCH.into());
        assert!(matches!(report.value.total_usage, PlatformResult::Error(_)));
        for (index, expected) in [(0, 0.0), (1, 100.0)] {
            let cpu = &report.value.logical_processors[index];
            assert_eq!(cpu.index, index);
            assert_eq!(cpu.name, format!("cpu{index}"));
            let PlatformResult::Success(value) = &cpu.usage else {
                panic!("expected usage")
            };
            assert_eq!(value.value.get(), expected);
            assert_eq!(value.source, DataSource::Measured);
            assert_eq!(value.timestamp, report.timestamp);
        }
        assert!(matches!(
            report.value.logical_processors[0].current_mhz,
            PlatformResult::Success(_)
        ));
        assert!(matches!(
            report.value.logical_processors[1].current_mhz,
            PlatformResult::Unavailable(_)
        ));
        assert!(matches!(
            report.value.logical_processors[2].usage,
            PlatformResult::Error(_)
        ));
    }

    #[test]
    fn early_requests_preserve_baseline_instead_of_refreshing_or_waiting() {
        let mut sampler = CpuSampler::default();
        let baseline = Instant::now();
        // First call initializes and must not invent a 0% measurement.
        assert!(matches!(
            sampler.backend.sample_at(baseline),
            PlatformResult::Unavailable(_)
        ));
        assert!(matches!(
            sampler.backend.sample_at(baseline),
            PlatformResult::Unavailable(_)
        ));
    }
}

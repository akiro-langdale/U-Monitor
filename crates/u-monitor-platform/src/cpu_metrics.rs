use std::time::{Duration, Instant, SystemTime};
use sysinfo::{MINIMUM_CPU_UPDATE_INTERVAL, System};
use u_monitor_core::{
    CpuMetrics, CpuUsagePercent, DataSource, LogicalCpuUsage, Measurement, PlatformResult,
    Timestamp,
};

#[derive(Default)]
pub(crate) struct CpuSampler {
    system: System,
    last_refresh: Option<Instant>,
}

impl CpuSampler {
    pub(crate) fn sample(&mut self) -> PlatformResult<CpuMetrics> {
        self.sample_at(Instant::now())
    }

    fn sample_at(&mut self, now: Instant) -> PlatformResult<CpuMetrics> {
        if !sysinfo::IS_SUPPORTED_SYSTEM {
            return PlatformResult::Unavailable(
                "sysinfo does not support this operating system".into(),
            );
        }
        if self
            .last_refresh
            .is_some_and(|last| now.duration_since(last) < MINIMUM_CPU_UPDATE_INTERVAL)
        {
            // Do not refresh or move the baseline on a too-early request.
            return PlatformResult::Unavailable(
                "CPU sampling interval is shorter than the backend minimum".into(),
            );
        }
        self.system.refresh_cpu_usage();
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
        from_report(
            self.system.global_cpu_usage(),
            self.system
                .cpus()
                .iter()
                .map(|cpu| (cpu.name().to_owned(), cpu.cpu_usage())),
            completed.duration_since(previous),
            SystemTime::now().into(),
        )
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

fn from_report(
    total: f32,
    processors: impl IntoIterator<Item = (String, f32)>,
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
                .map(|(index, (name, value))| LogicalCpuUsage {
                    index,
                    name,
                    usage: percent(value, timestamp),
                })
                .collect(),
        },
        timestamp,
    )
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
                ("cpu0".into(), 0.0),
                ("cpu1".into(), 100.0),
                ("cpu2".into(), -1.0),
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
            report.value.logical_processors[2].usage,
            PlatformResult::Error(_)
        ));
    }

    #[test]
    fn early_requests_preserve_baseline_instead_of_refreshing_or_waiting() {
        if !sysinfo::IS_SUPPORTED_SYSTEM || MINIMUM_CPU_UPDATE_INTERVAL.is_zero() {
            return;
        }
        let baseline = Instant::now();
        let mut sampler = CpuSampler {
            system: System::new(),
            last_refresh: Some(baseline),
        };
        assert!(matches!(
            sampler.sample_at(baseline),
            PlatformResult::Unavailable(_)
        ));
        assert_eq!(sampler.last_refresh, Some(baseline));
        assert!(sampler.system.cpus().is_empty());
    }
}

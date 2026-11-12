use std::{
    collections::VecDeque,
    time::{Duration, UNIX_EPOCH},
};
use u_monitor_core::{
    Availability, CpuMetrics, CpuUsagePercent, DataSource, DiagnosticError, LogicalCpuUsage,
    Measurement, PlatformAdapter, PlatformFeature, PlatformResult, RamInfo,
};
use u_monitor_telemetry::{TelemetryCollector, TelemetryWorker, WorkerConfig};

struct Scripted {
    cpu: VecDeque<PlatformResult<CpuMetrics>>,
    ram_available: bool,
}
impl PlatformAdapter for Scripted {
    fn name(&self) -> &'static str {
        "Scripted CPU"
    }
    fn check_availability(&self, _: PlatformFeature) -> Availability {
        Availability::Supported
    }
    fn cpu_metrics(&mut self) -> PlatformResult<CpuMetrics> {
        self.cpu
            .pop_front()
            .unwrap_or_else(|| PlatformResult::Unavailable("script exhausted".into()))
    }
    fn ram_info(&self) -> PlatformResult<RamInfo> {
        if !self.ram_available {
            return PlatformResult::Unavailable("RAM fixture".into());
        }
        let field = |value| {
            PlatformResult::Success(Measurement {
                value,
                source: DataSource::Measured,
                timestamp: UNIX_EPOCH.into(),
            })
        };
        PlatformResult::Success(Measurement {
            value: RamInfo {
                total_bytes: field(100),
                available_bytes: field(100),
                used_bytes: field(0),
            },
            source: DataSource::Measured,
            timestamp: UNIX_EPOCH.into(),
        })
    }
}

fn success(value: f32) -> PlatformResult<CpuMetrics> {
    let usage = PlatformResult::Success(Measurement {
        value: CpuUsagePercent::try_from(value).unwrap(),
        source: DataSource::Measured,
        timestamp: UNIX_EPOCH.into(),
    });
    PlatformResult::Success(Measurement {
        value: CpuMetrics {
            sampling_interval: Duration::from_secs(1),
            total_usage: usage.clone(),
            logical_processors: vec![LogicalCpuUsage {
                index: 0,
                name: "cpu0".into(),
                usage,
                current_mhz: PlatformResult::Success(Measurement {
                    value: 3200,
                    source: DataSource::Measured,
                    timestamp: UNIX_EPOCH.into(),
                }),
                max_mhz: PlatformResult::Success(Measurement {
                    value: 4800,
                    source: DataSource::Measured,
                    timestamp: UNIX_EPOCH.into(),
                }),
            }],
        },
        source: DataSource::Measured,
        timestamp: UNIX_EPOCH.into(),
    })
}

#[test]
fn cpu_warmup_failure_and_recovery_are_retained_without_reusing_previous_data() {
    let responses = [
        PlatformResult::Unavailable("warming up".into()),
        success(0.0),
        PlatformResult::Error(DiagnosticError::PlatformError("CPU failed".into())),
        PlatformResult::PermissionRequired("CPU denied".into()),
        success(75.0),
    ];
    let mut collector = TelemetryCollector::new(
        Scripted {
            cpu: responses.clone().into(),
            ram_available: true,
        },
        3,
    )
    .unwrap();
    for (index, expected) in responses.iter().enumerate() {
        let snapshot = collector.sample().unwrap();
        assert_eq!(&snapshot.cpu, expected);
        assert_eq!(snapshot.sequence, index as u64 + 1);
        assert!(matches!(snapshot.ram, PlatformResult::Success(_)));
    }
    assert_eq!(collector.history().len(), 3);
    assert_eq!(collector.history()[0].cpu, responses[2]);
    assert_eq!(collector.history()[2].cpu, responses[4]);
}

#[test]
fn worker_owns_cpu_state_and_delivers_it_with_independent_ram_outcomes() {
    let responses = [PlatformResult::Unavailable("warmup".into()), success(25.0)];
    let worker = TelemetryWorker::start(
        Scripted {
            cpu: responses.clone().into(),
            ram_available: false,
        },
        WorkerConfig {
            interval: Duration::from_secs(3600),
            history_capacity: 2,
            channel_capacity: 2,
        },
    )
    .unwrap();
    let first = worker.recv_timeout(Duration::from_secs(3)).unwrap();
    assert_eq!(first.cpu, responses[0]);
    worker.set_interval(Duration::from_millis(100)).unwrap();
    let second = worker.recv_timeout(Duration::from_secs(3)).unwrap();
    worker.request_stop();
    assert_eq!(second.cpu, responses[1]);
    assert!(matches!(second.ram, PlatformResult::Unavailable(_)));
    let report = worker.stop().unwrap();
    assert!(report.result.is_ok());
    assert!(report.history.iter().any(|sample| sample == &second));
}

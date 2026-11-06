use std::time::Duration;
use u_monitor_core::{DataSource, PlatformResult};
use u_monitor_platform::{MINIMUM_CPU_SAMPLING_INTERVAL, NativeTelemetryAdapter};
use u_monitor_telemetry::{TelemetryWorker, WorkerConfig};

#[test]
#[ignore = "samples real host CPU and RAM in a background worker; run on target environment"]
fn native_worker_warms_up_then_reports_cpu_and_ram() {
    let interval = MINIMUM_CPU_SAMPLING_INTERVAL.max(Duration::from_millis(250));
    let worker = TelemetryWorker::start(
        NativeTelemetryAdapter::default(),
        WorkerConfig {
            interval,
            history_capacity: 4,
            channel_capacity: 4,
        },
    )
    .expect("start worker");
    let first = worker
        .recv_timeout(Duration::from_secs(10))
        .expect("first attempt");
    assert!(matches!(first.cpu, PlatformResult::Unavailable(_)));
    let second = worker
        .recv_timeout(Duration::from_secs(10))
        .expect("second attempt");
    let PlatformResult::Success(cpu) = &second.cpu else {
        panic!(
            "target acceptance requires actual CPU data: {:?}",
            second.cpu
        )
    };
    assert_eq!(cpu.source, DataSource::Measured);
    assert!(cpu.value.sampling_interval >= MINIMUM_CPU_SAMPLING_INTERVAL);
    assert!(!cpu.value.logical_processors.is_empty());
    let PlatformResult::Success(total) = &cpu.value.total_usage else {
        panic!("total CPU missing")
    };
    assert_eq!(total.source, DataSource::Measured);
    assert_eq!(total.timestamp, cpu.timestamp);
    for processor in &cpu.value.logical_processors {
        let PlatformResult::Success(usage) = &processor.usage else {
            panic!("logical CPU missing")
        };
        assert!((0.0..=100.0).contains(&usage.value.get()));
        assert_eq!(usage.timestamp, cpu.timestamp);
        assert_eq!(usage.source, DataSource::Measured);
    }
    assert!(matches!(second.ram, PlatformResult::Success(_)));
    eprintln!(
        "CPU {:.2}% · {} logical processors · interval {:?} · UTC {}",
        total.value.get(),
        cpu.value.logical_processors.len(),
        cpu.value.sampling_interval,
        cpu.timestamp
    );
    let report = worker.stop().expect("join");
    assert!(report.result.is_ok());
    assert!(report.history.iter().any(|sample| sample == &second));
}

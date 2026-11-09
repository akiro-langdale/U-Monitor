use std::time::{Duration, Instant};
use u_monitor_core::{Availability, PlatformAdapter, PlatformFeature, PlatformResult, RamInfo};
use u_monitor_telemetry::{TelemetryWorker, WorkerConfig};

struct QuickAdapter;

impl PlatformAdapter for QuickAdapter {
    fn name(&self) -> &'static str {
        "Quick jitter guard"
    }

    fn check_availability(&self, _: PlatformFeature) -> Availability {
        Availability::Supported
    }

    fn ram_info(&self) -> PlatformResult<RamInfo> {
        PlatformResult::Unavailable("jitter guard: no RAM payload".into())
    }
}

#[test]
fn delivery_respects_configured_interval() {
    let interval = Duration::from_millis(80);
    let worker = TelemetryWorker::start(
        QuickAdapter,
        WorkerConfig {
            interval,
            history_capacity: 8,
            channel_capacity: 4,
        },
    )
    .expect("worker starts");

    let mut deliveries = Vec::new();
    let deadline = Instant::now() + interval * 4 + Duration::from_millis(200);
    while deliveries.len() < 3 && Instant::now() < deadline {
        if worker.recv_timeout(Duration::from_millis(150)).is_ok() {
            deliveries.push(Instant::now());
        }
    }
    worker.request_stop();
    let _ = worker.stop();

    assert!(
        deliveries.len() >= 3,
        "expected at least three deliveries, got {}",
        deliveries.len()
    );
    for pair in deliveries[1..].windows(2) {
        let delta = pair[1].saturating_duration_since(pair[0]);
        assert!(
            delta >= interval * 3 / 4,
            "samples after warmup arrived too quickly: {:?}",
            delta
        );
    }
}

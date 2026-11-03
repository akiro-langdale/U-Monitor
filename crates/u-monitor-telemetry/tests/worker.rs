use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, Sender},
    },
    time::Duration,
};
use u_monitor_core::{
    Availability, DiagnosticError, PlatformAdapter, PlatformFeature, PlatformResult, RamInfo,
};
use u_monitor_telemetry::{TelemetryWorker, WorkerConfig};

const WAIT: Duration = Duration::from_secs(3);

fn config(interval: Duration) -> WorkerConfig {
    WorkerConfig {
        interval,
        history_capacity: 2,
        channel_capacity: 1,
    }
}

struct GateAdapter {
    entered: Sender<()>,
    release: Receiver<PlatformResult<RamInfo>>,
}

impl PlatformAdapter for GateAdapter {
    fn name(&self) -> &'static str {
        "Gated test adapter"
    }
    fn check_availability(&self, _: PlatformFeature) -> Availability {
        Availability::Supported
    }
    fn ram_info(&self) -> PlatformResult<RamInfo> {
        self.entered
            .send(())
            .expect("test receives adapter signals");
        // Timeout permits teardown even if a test assertion fails before release.
        self.release.recv_timeout(WAIT).unwrap_or_else(|_| {
            PlatformResult::Error(DiagnosticError::PlatformError("test gate timed out".into()))
        })
    }
}

fn start_gated(
    interval: Duration,
) -> (
    TelemetryWorker,
    Receiver<()>,
    Sender<PlatformResult<RamInfo>>,
) {
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let worker = TelemetryWorker::start(
        GateAdapter {
            entered: entered_tx,
            release: release_rx,
        },
        config(interval),
    )
    .expect("worker starts");
    (worker, entered_rx, release_tx)
}

fn entered(receiver: &Receiver<()>) {
    receiver.recv_timeout(WAIT).expect("worker reached adapter");
}

fn missing() -> PlatformResult<RamInfo> {
    PlatformResult::Unavailable("test fixture: no RAM report".into())
}

#[test]
fn zero_configuration_is_rejected_before_starting() {
    for invalid in [
        config(Duration::ZERO),
        WorkerConfig {
            history_capacity: 0,
            ..config(Duration::from_secs(1))
        },
        WorkerConfig {
            channel_capacity: 0,
            ..config(Duration::from_secs(1))
        },
        config(Duration::MAX),
    ] {
        let (entered_tx, entered_rx) = mpsc::channel();
        let (_release_tx, release_rx) = mpsc::channel();
        assert!(matches!(
            TelemetryWorker::start(
                GateAdapter {
                    entered: entered_tx,
                    release: release_rx
                },
                invalid,
            ),
            Err(DiagnosticError::ValidationError(_))
        ));
        assert!(entered_rx.try_recv().is_err());
    }
}

#[test]
fn first_sample_is_immediate_and_stop_preserves_inflight_result() {
    let (worker, signals, release) = start_gated(Duration::from_secs(3600));
    entered(&signals);
    worker.request_stop();
    worker.request_stop();
    let response = PlatformResult::PermissionRequired("test access denied".into());
    release.send(response.clone()).expect("release adapter");
    let report = worker.stop().expect("join");
    assert!(report.result.is_ok());
    assert_eq!(report.samples_collected, 1);
    assert_eq!(report.history.len(), 1);
    assert_eq!(report.history[0].ram, response);
}

#[test]
fn channel_overflow_is_counted_while_history_keeps_newest_attempts() {
    let (worker, signals, release) = start_gated(Duration::from_nanos(1));
    entered(&signals);
    release.send(missing()).expect("first release");
    entered(&signals); // First notification is now queued.
    release.send(missing()).expect("second release");
    entered(&signals); // Second notification was rejected by the full queue.
    assert_eq!(worker.dropped_notifications(), 1);
    worker.request_stop();
    release.send(missing()).expect("third release");
    let report = worker.stop().expect("join");
    assert!(report.result.is_ok());
    assert_eq!(report.samples_collected, 3);
    assert_eq!(report.dropped_notifications, 2);
    assert_eq!(
        report
            .history
            .iter()
            .map(|item| item.sequence)
            .collect::<Vec<_>>(),
        [2, 3]
    );
}

#[test]
fn interval_change_wakes_worker_and_adapter_errors_do_not_stop_sampling() {
    let (worker, signals, release) = start_gated(Duration::from_secs(3600));
    entered(&signals);
    let failure = PlatformResult::Error(DiagnosticError::PlatformError("test API failure".into()));
    release.send(failure.clone()).expect("first release");
    let first = worker.recv_timeout(WAIT).expect("first notification");
    assert_eq!(first.ram, failure);
    assert!(worker.set_interval(Duration::ZERO).is_err());
    assert!(worker.set_interval(Duration::MAX).is_err());
    worker
        .set_interval(Duration::from_nanos(1))
        .expect("valid new interval");
    entered(&signals);
    worker.request_stop();
    assert!(worker.set_interval(Duration::from_secs(1)).is_err());
    release.send(missing()).expect("second release");
    let report = worker.stop().expect("join");
    assert_eq!(report.samples_collected, 2);
    assert_eq!(report.history[0].ram, failure);
    assert!(matches!(
        report.history[1].ram,
        PlatformResult::Unavailable(_)
    ));
}

struct DropAdapter(Arc<AtomicBool>);
impl PlatformAdapter for DropAdapter {
    fn name(&self) -> &'static str {
        "Drop test adapter"
    }
    fn check_availability(&self, _: PlatformFeature) -> Availability {
        Availability::NotImplemented
    }
}
impl Drop for DropAdapter {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

#[test]
fn dropping_handle_joins_worker_and_releases_owned_adapter() {
    let dropped = Arc::new(AtomicBool::new(false));
    let worker = TelemetryWorker::start(
        DropAdapter(Arc::clone(&dropped)),
        config(Duration::from_secs(3600)),
    )
    .expect("start");
    worker.recv_timeout(WAIT).expect("first snapshot");
    // If Drop waited out the interval this test could not complete.
    drop(worker);
    assert!(dropped.load(Ordering::SeqCst));
}

#[test]
fn explicit_stop_interrupts_wait_without_waiting_for_next_sample() {
    let worker = TelemetryWorker::start(
        DropAdapter(Arc::new(AtomicBool::new(false))),
        config(Duration::from_secs(3600)),
    )
    .expect("start");
    worker.recv_timeout(WAIT).expect("first snapshot");
    let report = worker.stop().expect("join");
    assert!(report.result.is_ok());
    assert_eq!(report.samples_collected, 1);
    assert_eq!(report.dropped_notifications, 0);
}

struct PanicAdapter;
impl PlatformAdapter for PanicAdapter {
    fn name(&self) -> &'static str {
        "Panic test adapter"
    }
    fn check_availability(&self, _: PlatformFeature) -> Availability {
        Availability::Error
    }
    fn ram_info(&self) -> PlatformResult<RamInfo> {
        panic!("intentional worker panic for join test")
    }
}

#[test]
fn worker_panic_is_reported_without_panicking_the_caller() {
    let worker =
        TelemetryWorker::start(PanicAdapter, config(Duration::from_secs(1))).expect("start");
    assert!(matches!(
        worker.recv_timeout(WAIT),
        Err(mpsc::RecvTimeoutError::Disconnected)
    ));
    assert!(matches!(
        worker.stop(),
        Err(DiagnosticError::StorageError(_))
    ));
}

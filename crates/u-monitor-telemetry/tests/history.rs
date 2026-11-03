use std::{cell::RefCell, collections::VecDeque, time::UNIX_EPOCH};
use u_monitor_core::{
    Availability, DataSource, DiagnosticError, Measurement, PlatformAdapter, PlatformFeature,
    PlatformResult, RamInfo,
};
use u_monitor_telemetry::TelemetryCollector;

struct ScriptedAdapter {
    responses: RefCell<VecDeque<PlatformResult<RamInfo>>>,
}

impl ScriptedAdapter {
    fn new(responses: impl IntoIterator<Item = PlatformResult<RamInfo>>) -> Self {
        Self {
            responses: RefCell::new(responses.into_iter().collect()),
        }
    }
}

impl PlatformAdapter for ScriptedAdapter {
    fn name(&self) -> &'static str {
        "Scripted test adapter"
    }
    fn check_availability(&self, _: PlatformFeature) -> Availability {
        Availability::Supported
    }
    fn ram_info(&self) -> PlatformResult<RamInfo> {
        self.responses
            .borrow_mut()
            .pop_front()
            .expect("unexpected additional sampling")
    }
}

fn ram(value: u64) -> PlatformResult<RamInfo> {
    let field = |value, source| {
        PlatformResult::Success(Measurement {
            value,
            source,
            timestamp: UNIX_EPOCH.into(),
        })
    };
    PlatformResult::Success(Measurement {
        value: RamInfo {
            total_bytes: field(value, DataSource::Measured),
            available_bytes: field(0, DataSource::Measured),
            used_bytes: field(value, DataSource::Calculated),
        },
        source: DataSource::Measured,
        timestamp: UNIX_EPOCH.into(),
    })
}

#[test]
fn zero_capacity_is_rejected() {
    assert!(matches!(
        TelemetryCollector::new(ScriptedAdapter::new([]), 0),
        Err(DiagnosticError::ValidationError(_))
    ));
}

#[test]
fn construction_does_not_sample_and_history_starts_empty() {
    let collector = TelemetryCollector::new(ScriptedAdapter::new([]), 3).expect("valid capacity");
    assert_eq!(collector.capacity(), 3);
    assert!(collector.history().is_empty());
}

#[test]
fn retained_snapshots_are_ordered_and_oldest_is_evicted() {
    let mut collector =
        TelemetryCollector::new(ScriptedAdapter::new([ram(1), ram(2), ram(3), ram(4)]), 2)
            .expect("valid capacity");
    let mut previous_elapsed = std::time::Duration::ZERO;
    for sequence in 1..=4 {
        let sample = collector.sample().expect("sample");
        assert_eq!(sample.sequence, sequence);
        assert!(sample.elapsed >= previous_elapsed);
        previous_elapsed = sample.elapsed;
        assert_eq!(collector.history().back(), Some(&sample));
        assert!(collector.history().len() <= collector.capacity());
    }
    let retained: Vec<_> = collector
        .history()
        .iter()
        .map(|snapshot| snapshot.sequence)
        .collect();
    assert_eq!(retained, [3, 4]);
}

#[test]
fn capacity_one_replaces_the_previous_attempt() {
    let mut collector = TelemetryCollector::new(ScriptedAdapter::new([ram(1024), ram(2048)]), 1)
        .expect("valid capacity");
    collector.sample().expect("first");
    let second = collector.sample().expect("second");
    assert_eq!(collector.history().len(), 1);
    assert_eq!(collector.history().front(), Some(&second));
}

#[test]
fn failures_are_retained_without_reusing_success_and_sampling_can_recover() {
    let responses = [
        ram(4096),
        PlatformResult::Error(DiagnosticError::PlatformError("native API failed".into())),
        PlatformResult::PermissionRequired("access denied".into()),
        PlatformResult::Unavailable("not supported".into()),
        PlatformResult::NotImplemented("not implemented".into()),
        ram(8192),
    ];
    let expected = responses.clone();
    let mut collector =
        TelemetryCollector::new(ScriptedAdapter::new(responses), 6).expect("valid capacity");
    for response in expected {
        let snapshot = collector
            .sample()
            .expect("adapter errors are snapshot data");
        assert_eq!(snapshot.ram, response);
    }
    assert_eq!(collector.history().len(), 6);
    assert!(matches!(
        collector.history()[1].ram,
        PlatformResult::Error(_)
    ));
    assert!(matches!(
        collector.history()[5].ram,
        PlatformResult::Success(_)
    ));
}

#[test]
fn adapter_timestamps_and_partial_field_errors_are_preserved() {
    let response = PlatformResult::Success(Measurement {
        value: RamInfo {
            total_bytes: PlatformResult::Error(DiagnosticError::ValidationError(
                "invalid total".into(),
            )),
            available_bytes: PlatformResult::Unavailable("missing".into()),
            used_bytes: PlatformResult::NotImplemented("pending".into()),
        },
        source: DataSource::Measured,
        timestamp: UNIX_EPOCH.into(),
    });
    let mut collector = TelemetryCollector::new(ScriptedAdapter::new([response.clone()]), 1)
        .expect("valid capacity");
    let snapshot = collector.sample().expect("snapshot");
    assert_eq!(snapshot.ram, response);
    assert!(snapshot.captured_at > UNIX_EPOCH.into());
    // Modifying the caller's owned snapshot cannot corrupt retained history.
    let mut modified = snapshot.clone();
    modified.ram = PlatformResult::Unavailable("caller change".into());
    assert_eq!(collector.history().back(), Some(&snapshot));
}

//! Cross-cutting provenance invariants for Measurement / DataSource.

use std::time::UNIX_EPOCH;
use u_monitor_core::{DataSource, Measurement, PlatformResult};

#[test]
fn simulated_measurements_are_distinct_from_measured() {
    let measured = PlatformResult::Success(Measurement {
        value: 1u64,
        source: DataSource::Measured,
        timestamp: UNIX_EPOCH.into(),
    });
    let simulated = PlatformResult::Success(Measurement {
        value: 1u64,
        source: DataSource::Simulated,
        timestamp: UNIX_EPOCH.into(),
    });
    let PlatformResult::Success(a) = measured else {
        panic!("measured");
    };
    let PlatformResult::Success(b) = simulated else {
        panic!("simulated");
    };
    assert_eq!(a.value, b.value);
    assert_ne!(a.source, b.source);
}

#[test]
fn unavailable_is_not_a_zero_measurement() {
    let missing: PlatformResult<u64> = PlatformResult::Unavailable("no sensor".into());
    assert!(!matches!(missing, PlatformResult::Success(_)));
}

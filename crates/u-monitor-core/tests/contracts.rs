use std::error::Error;
use std::time::{Duration, UNIX_EPOCH};
use u_monitor_core::{Availability, DataSource, DiagnosticError, Measurement, PlatformResult};

#[test]
fn availability_labels_match_external_contract() {
    for (state, label) in [
        (Availability::Supported, "SUPPORTED"),
        (Availability::PermissionRequired, "PERMISSION_REQUIRED"),
        (Availability::Unavailable, "UNAVAILABLE"),
        (Availability::NotImplemented, "NOT_IMPLEMENTED"),
        (Availability::Error, "ERROR"),
    ] {
        assert_eq!(state.to_string(), label);
    }
}

#[test]
fn success_preserves_value_origin_and_acquisition_time() {
    let timestamp = (UNIX_EPOCH + Duration::from_secs(1_700_000_000)).into();
    for source in [
        DataSource::Measured,
        DataSource::Calculated,
        DataSource::Simulated,
    ] {
        let result = PlatformResult::Success(Measurement {
            value: vec![0_u8, 127, 255],
            source,
            timestamp,
        });
        assert_eq!(result.availability(), Availability::Supported);
        let PlatformResult::Success(measurement) = result else {
            panic!("successful request lost its measurement");
        };
        assert_eq!(measurement.value, [0, 127, 255]);
        assert_eq!(measurement.source, source);
        assert_eq!(measurement.timestamp, timestamp);
    }
}

#[test]
fn missing_data_is_distinct_from_execution_failure() {
    let cases: [(PlatformResult<u64>, Availability); 4] = [
        (
            PlatformResult::PermissionRequired("Access denied".into()),
            Availability::PermissionRequired,
        ),
        (
            PlatformResult::Unavailable("No supported API".into()),
            Availability::Unavailable,
        ),
        (
            PlatformResult::NotImplemented("Adapter pending".into()),
            Availability::NotImplemented,
        ),
        (
            PlatformResult::Error(DiagnosticError::PlatformError("Native call failed".into())),
            Availability::Error,
        ),
    ];
    for (result, expected) in cases {
        assert_eq!(result.availability(), expected);
        assert!(!matches!(result, PlatformResult::Success(_)));
    }
}

#[test]
fn diagnostics_are_standard_errors_and_retain_context() {
    let cases = [
        (
            DiagnosticError::PlatformError("read failed".into()),
            "Platform adapter error: read failed",
        ),
        (
            DiagnosticError::StorageError("full".into()),
            "Telemetry storage error: full",
        ),
        (
            DiagnosticError::SimulationError("invalid model".into()),
            "Simulation failure in model: invalid model",
        ),
        (
            DiagnosticError::ValidationError("bad interval".into()),
            "Data validation failed: bad interval",
        ),
        (DiagnosticError::Unknown, "Unknown hardware or system error"),
    ];
    for (error, expected) in cases {
        let standard_error: &dyn Error = &error;
        assert_eq!(standard_error.to_string(), expected);
        assert!(standard_error.source().is_none());
    }
}

#[test]
fn floating_point_metrics_do_not_require_eq() {
    let result = PlatformResult::Success(Measurement {
        value: 12.5_f64,
        source: DataSource::Calculated,
        timestamp: UNIX_EPOCH.into(),
    });
    assert_eq!(result, result.clone());
}

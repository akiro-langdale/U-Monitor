//! Refresh only RAM. A fresh backend prevents a failed refresh reusing stale data.
use std::time::SystemTime;
use sysinfo::{MemoryRefreshKind, System};
use u_monitor_core::{
    DataSource, DiagnosticError, Measurement, PlatformResult, RamInfo, Timestamp,
};

pub(crate) fn discover() -> PlatformResult<RamInfo> {
    if !sysinfo::IS_SUPPORTED_SYSTEM {
        return PlatformResult::Unavailable(
            "sysinfo does not support this operating system".into(),
        );
    }
    let mut system = System::new();
    system.refresh_memory_specifics(MemoryRefreshKind::nothing().with_ram());
    let total = system.total_memory();
    // This backend does not expose individual field errors. A zero total from
    // a fresh instance is not evidence of a valid zero available-memory value.
    if total == 0 {
        return PlatformResult::Unavailable("RAM statistics were not reported by sysinfo".into());
    }
    from_report(
        Some(total),
        Some(system.available_memory()),
        SystemTime::now().into(),
    )
}

fn value<T>(value: T, source: DataSource, timestamp: Timestamp) -> PlatformResult<T> {
    PlatformResult::Success(Measurement {
        value,
        source,
        timestamp,
    })
}

fn missing(reason: &str) -> PlatformResult<u64> {
    PlatformResult::Unavailable(reason.into())
}

fn invalid(reason: &str) -> PlatformResult<u64> {
    PlatformResult::Error(DiagnosticError::ValidationError(reason.into()))
}

fn from_report(
    total: Option<u64>,
    available: Option<u64>,
    timestamp: Timestamp,
) -> PlatformResult<RamInfo> {
    let total = total.filter(|bytes| *bytes > 0);
    if total.is_none() && available.is_none() {
        return PlatformResult::Unavailable("No RAM statistics were reported".into());
    }
    let total_bytes = match total {
        Some(bytes) => value(bytes, DataSource::Measured, timestamp),
        None => missing("Total RAM was not reported"),
    };
    let available_bytes = match (total, available) {
        (Some(total), Some(available)) if available > total => {
            invalid("Available RAM exceeds total RAM")
        }
        (_, Some(bytes)) => value(bytes, DataSource::Measured, timestamp),
        (_, None) => missing("Available RAM was not reported"),
    };
    let used_bytes = match (total, available) {
        (Some(total), Some(available)) => match total.checked_sub(available) {
            Some(bytes) => value(bytes, DataSource::Calculated, timestamp),
            None => invalid("Cannot calculate used RAM: available exceeds total"),
        },
        _ => missing("Used RAM requires both total and available RAM"),
    };
    value(
        RamInfo {
            total_bytes,
            available_bytes,
            used_bytes,
        },
        DataSource::Measured,
        timestamp,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::UNIX_EPOCH;

    fn snapshot(total: Option<u64>, available: Option<u64>) -> RamInfo {
        let PlatformResult::Success(snapshot) = from_report(total, available, UNIX_EPOCH.into())
        else {
            panic!("expected at least one reported field");
        };
        snapshot.value
    }

    #[test]
    fn complete_snapshot_distinguishes_reported_and_calculated_fields() {
        let info = snapshot(Some(16_384), Some(4_096));
        for (field, expected, source) in [
            (info.total_bytes, 16_384, DataSource::Measured),
            (info.available_bytes, 4_096, DataSource::Measured),
            (info.used_bytes, 12_288, DataSource::Calculated),
        ] {
            let PlatformResult::Success(field) = field else {
                panic!("missing field")
            };
            assert_eq!(field.value, expected);
            assert_eq!(field.source, source);
            assert_eq!(field.timestamp, UNIX_EPOCH.into());
        }
    }

    #[test]
    fn exhausted_and_fully_available_memory_are_valid() {
        for available in [0, 1024] {
            let info = snapshot(Some(1024), Some(available));
            let PlatformResult::Success(field) = info.available_bytes else {
                panic!("valid available value")
            };
            assert_eq!(field.value, available);
            let PlatformResult::Success(field) = info.used_bytes else {
                panic!("valid used value")
            };
            assert_eq!(field.value, 1024 - available);
        }
    }

    #[test]
    fn missing_available_memory_does_not_become_zero_or_hide_total() {
        let info = snapshot(Some(1024), None);
        assert!(matches!(info.total_bytes, PlatformResult::Success(_)));
        assert!(matches!(
            info.available_bytes,
            PlatformResult::Unavailable(_)
        ));
        assert!(matches!(info.used_bytes, PlatformResult::Unavailable(_)));
    }

    #[test]
    fn missing_total_preserves_available_without_inventing_used() {
        for total in [None, Some(0)] {
            let info = snapshot(total, Some(512));
            assert!(matches!(info.total_bytes, PlatformResult::Unavailable(_)));
            assert!(matches!(info.available_bytes, PlatformResult::Success(_)));
            assert!(matches!(info.used_bytes, PlatformResult::Unavailable(_)));
        }
    }

    #[test]
    fn inconsistent_report_is_an_error_without_underflow() {
        let info = snapshot(Some(1024), Some(u64::MAX));
        assert!(matches!(info.total_bytes, PlatformResult::Success(_)));
        assert!(matches!(
            info.available_bytes,
            PlatformResult::Error(DiagnosticError::ValidationError(_))
        ));
        assert!(matches!(
            info.used_bytes,
            PlatformResult::Error(DiagnosticError::ValidationError(_))
        ));
        let info = snapshot(Some(u64::MAX), Some(0));
        let PlatformResult::Success(used) = info.used_bytes else {
            panic!("valid u64 maximum")
        };
        assert_eq!(used.value, u64::MAX);
    }

    #[test]
    fn absent_report_does_not_create_a_measurement() {
        for total in [None, Some(0)] {
            assert!(matches!(
                from_report(total, None, UNIX_EPOCH.into()),
                PlatformResult::Unavailable(_)
            ));
        }
    }
}

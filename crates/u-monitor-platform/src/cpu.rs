//! sysinfo is confined to this adapter; no process or memory refresh is requested.
use std::{num::NonZeroUsize, time::SystemTime};
use sysinfo::{CpuRefreshKind, System};
use u_monitor_core::{CpuInfo, DataSource, Measurement, PlatformResult, Timestamp};

pub(crate) fn discover() -> PlatformResult<CpuInfo> {
    if !sysinfo::IS_SUPPORTED_SYSTEM {
        return PlatformResult::Unavailable(
            "sysinfo does not support this operating system".into(),
        );
    }
    let mut system = System::new();
    system.refresh_cpu_list(CpuRefreshKind::nothing());
    let physical_cores = System::physical_core_count();
    from_report(
        system.cpus().iter().map(|cpu| cpu.brand()),
        system.cpus().len(),
        physical_cores,
        SystemTime::now().into(),
    )
}

fn measured<T>(value: T, timestamp: Timestamp) -> PlatformResult<T> {
    PlatformResult::Success(Measurement {
        value,
        source: DataSource::Measured,
        timestamp,
    })
}

fn count(value: Option<usize>, label: &str, timestamp: Timestamp) -> PlatformResult<NonZeroUsize> {
    match value.and_then(NonZeroUsize::new) {
        Some(value) => measured(value, timestamp),
        None => PlatformResult::Unavailable(format!("{label} was not reported by the OS")),
    }
}

// The backend returns empty/None for some failures, so do not invent native
// error codes or claim that elevation would recover missing data.
fn from_report<'a>(
    brands: impl IntoIterator<Item = &'a str>,
    logical: usize,
    physical: Option<usize>,
    timestamp: Timestamp,
) -> PlatformResult<CpuInfo> {
    let mut models = Vec::new();
    for brand in brands {
        let brand = brand.trim();
        if !brand.is_empty() && !models.iter().any(|model| model == brand) {
            models.push(brand.to_owned());
        }
    }
    let model_names = if models.is_empty() {
        PlatformResult::Unavailable("CPU model was not reported by the OS".into())
    } else {
        measured(models, timestamp)
    };
    let logical_processors = count(Some(logical), "Logical processor count", timestamp);
    let physical_cores = count(physical, "Physical core count", timestamp);
    if !matches!(model_names, PlatformResult::Success(_))
        && !matches!(logical_processors, PlatformResult::Success(_))
        && !matches!(physical_cores, PlatformResult::Success(_))
    {
        return PlatformResult::Unavailable("No CPU discovery data was reported by the OS".into());
    }
    measured(
        CpuInfo {
            model_names,
            logical_processors,
            physical_cores,
        },
        timestamp,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::UNIX_EPOCH;

    #[test]
    fn complete_report_preserves_distinct_models_counts_and_provenance() {
        let timestamp = UNIX_EPOCH.into();
        let PlatformResult::Success(snapshot) = from_report(
            [" Model A ", "Model A", "", "Model B"],
            16,
            Some(8),
            timestamp,
        ) else {
            panic!("expected CPU data")
        };
        assert_eq!(snapshot.source, DataSource::Measured);
        assert_eq!(snapshot.timestamp, timestamp);
        let PlatformResult::Success(models) = snapshot.value.model_names else {
            panic!("models missing")
        };
        assert_eq!(models.value, ["Model A", "Model B"]);
        assert_eq!(models.source, DataSource::Measured);
        assert_eq!(models.timestamp, timestamp);
        let PlatformResult::Success(logical) = snapshot.value.logical_processors else {
            panic!("logical count missing")
        };
        let PlatformResult::Success(physical) = snapshot.value.physical_cores else {
            panic!("physical count missing")
        };
        assert_eq!(logical.value.get(), 16);
        assert_eq!(physical.value.get(), 8);
    }

    #[test]
    fn missing_topology_keeps_model_without_estimating_core_counts() {
        let PlatformResult::Success(snapshot) = from_report(["Model"], 0, None, UNIX_EPOCH.into())
        else {
            panic!("model should remain available")
        };
        assert!(matches!(
            snapshot.value.model_names,
            PlatformResult::Success(_)
        ));
        assert!(matches!(
            snapshot.value.logical_processors,
            PlatformResult::Unavailable(_)
        ));
        assert!(matches!(
            snapshot.value.physical_cores,
            PlatformResult::Unavailable(_)
        ));
    }

    #[test]
    fn missing_model_keeps_available_counts_and_rejects_zero_physical_count() {
        let PlatformResult::Success(snapshot) =
            from_report([" ", ""], 12, Some(0), UNIX_EPOCH.into())
        else {
            panic!("logical count should remain available")
        };
        assert!(matches!(
            snapshot.value.model_names,
            PlatformResult::Unavailable(_)
        ));
        assert!(matches!(
            snapshot.value.logical_processors,
            PlatformResult::Success(_)
        ));
        assert!(matches!(
            snapshot.value.physical_cores,
            PlatformResult::Unavailable(_)
        ));
    }

    #[test]
    fn entirely_missing_report_has_no_fake_measurement() {
        assert!(matches!(
            from_report([], 0, None, UNIX_EPOCH.into()),
            PlatformResult::Unavailable(_)
        ));
    }
}

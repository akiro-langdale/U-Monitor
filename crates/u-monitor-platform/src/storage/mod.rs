//! Mounted-volume discovery, disk I/O rates, and read-only file samples.
mod file_hex;
mod io_rates;

#[cfg(target_os = "macos")]
mod macos_disks;

pub use file_hex::MAX_FILE_SAMPLE_BYTES;
pub(crate) use file_hex::read_file_bytes;
pub(crate) use io_rates::DiskIoSampler;
pub use io_rates::MINIMUM_INTERVAL as MINIMUM_DISK_IO_INTERVAL;

use std::{ffi::OsStr, time::SystemTime};
use sysinfo::{DiskKind, DiskRefreshKind, Disks};
use u_monitor_core::{
    DataSource, DiagnosticError, Measurement, PlatformResult, StorageKind, StorageVolume,
    Timestamp, VolumeSpace,
};

pub(crate) fn discover() -> PlatformResult<Vec<StorageVolume>> {
    if !sysinfo::IS_SUPPORTED_SYSTEM {
        return PlatformResult::Unavailable(
            "sysinfo does not support this operating system".into(),
        );
    }
    let disks = Disks::new_with_refreshed_list_specifics(
        DiskRefreshKind::nothing().with_kind().with_storage(),
    );
    // Enumeration has no Result in sysinfo 0.37.2. An empty list does not prove
    // there are no physical devices, nor identify permission failures.
    if disks.is_empty() {
        return PlatformResult::Unavailable("No mounted volumes were reported by sysinfo; enumeration failure cannot be distinguished from an empty list".into());
    }
    let timestamp = SystemTime::now().into();
    let volumes = disks
        .iter()
        .map(|disk| StorageVolume {
            name: reported_text(disk.name(), timestamp),
            mount_point: disk.mount_point().to_path_buf(),
            file_system: reported_text(disk.file_system(), timestamp),
            kind: match disk.kind() {
                DiskKind::HDD => StorageKind::Hdd,
                DiskKind::SSD => StorageKind::Ssd,
                DiskKind::Unknown(code) => StorageKind::Unknown(code),
            },
            space: space(disk.total_space(), disk.available_space(), timestamp),
        })
        .collect();
    measured(volumes, DataSource::Measured, timestamp)
}

fn measured<T>(value: T, source: DataSource, timestamp: Timestamp) -> PlatformResult<T> {
    PlatformResult::Success(Measurement {
        value,
        source,
        timestamp,
    })
}

fn reported_text(text: &OsStr, timestamp: Timestamp) -> PlatformResult<std::ffi::OsString> {
    if text.is_empty() {
        PlatformResult::Unavailable("Volume metadata was not reported".into())
    } else {
        measured(text.to_owned(), DataSource::Measured, timestamp)
    }
}

fn invalid(reason: &str) -> PlatformResult<u64> {
    PlatformResult::Error(DiagnosticError::ValidationError(reason.into()))
}

fn space(total: u64, available: u64, timestamp: Timestamp) -> VolumeSpace {
    // A fresh sysinfo disk with no space data may contain zeroes. Do not display
    // them as a measured empty/full volume or infer that elevation will help.
    if total == 0 {
        let missing =
            || PlatformResult::Unavailable("Volume space was not reported (zero total)".into());
        return VolumeSpace {
            total_bytes: missing(),
            available_bytes: missing(),
            used_bytes: missing(),
        };
    }
    let total_bytes = measured(total, DataSource::Measured, timestamp);
    let (available_bytes, used_bytes) = match total.checked_sub(available) {
        Some(used) => (
            measured(available, DataSource::Measured, timestamp),
            measured(used, DataSource::Calculated, timestamp),
        ),
        None => (
            invalid("Available volume space exceeds total space"),
            invalid("Cannot calculate used space: available exceeds total"),
        ),
    };
    VolumeSpace {
        total_bytes,
        available_bytes,
        used_bytes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::UNIX_EPOCH;

    #[test]
    fn space_snapshot_retains_bytes_time_and_calculated_provenance() {
        let timestamp = UNIX_EPOCH.into();
        let info = space(10_000, 3_000, timestamp);
        for (field, expected, source) in [
            (info.total_bytes, 10_000, DataSource::Measured),
            (info.available_bytes, 3_000, DataSource::Measured),
            (info.used_bytes, 7_000, DataSource::Calculated),
        ] {
            let PlatformResult::Success(field) = field else {
                panic!("space missing")
            };
            assert_eq!(field.value, expected);
            assert_eq!(field.source, source);
            assert_eq!(field.timestamp, timestamp);
        }
    }

    #[test]
    fn full_and_empty_volumes_have_valid_zero_fields() {
        for available in [0, 1000] {
            let info = space(1000, available, UNIX_EPOCH.into());
            let PlatformResult::Success(actual) = info.available_bytes else {
                panic!("available missing")
            };
            assert_eq!(actual.value, available);
            let PlatformResult::Success(actual) = info.used_bytes else {
                panic!("used missing")
            };
            assert_eq!(actual.value, 1000 - available);
        }
    }

    #[test]
    fn unknown_space_is_not_a_fake_zero_measurement() {
        let info = space(0, 0, UNIX_EPOCH.into());
        assert!(matches!(info.total_bytes, PlatformResult::Unavailable(_)));
        assert!(matches!(
            info.available_bytes,
            PlatformResult::Unavailable(_)
        ));
        assert!(matches!(info.used_bytes, PlatformResult::Unavailable(_)));
    }

    #[test]
    fn contradictory_space_keeps_total_but_returns_errors_without_underflow() {
        let info = space(1, u64::MAX, UNIX_EPOCH.into());
        assert!(matches!(info.total_bytes, PlatformResult::Success(_)));
        assert!(matches!(
            info.available_bytes,
            PlatformResult::Error(DiagnosticError::ValidationError(_))
        ));
        assert!(matches!(
            info.used_bytes,
            PlatformResult::Error(DiagnosticError::ValidationError(_))
        ));
    }

    #[test]
    fn unknown_text_is_unavailable_and_unicode_text_is_preserved() {
        assert!(matches!(
            reported_text(OsStr::new(""), UNIX_EPOCH.into()),
            PlatformResult::Unavailable(_)
        ));
        let text = OsStr::new("Данные 外付け");
        let PlatformResult::Success(actual) = reported_text(text, UNIX_EPOCH.into()) else {
            panic!("text missing")
        };
        assert_eq!(actual.value, text);
    }

    #[cfg(unix)]
    #[test]
    fn non_utf8_metadata_is_preserved_without_loss() {
        use std::os::unix::ffi::OsStrExt;
        let text = OsStr::from_bytes(&[100, 105, 115, 107, 45, 255]);
        let PlatformResult::Success(actual) = reported_text(text, UNIX_EPOCH.into()) else {
            panic!("text missing")
        };
        assert_eq!(actual.value.as_os_str().as_bytes(), text.as_bytes());
    }
}

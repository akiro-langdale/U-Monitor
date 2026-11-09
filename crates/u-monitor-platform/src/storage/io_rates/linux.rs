//! Aggregate bytes from `/proc/diskstats` (sectors × 512).

use std::{fs, io};
use u_monitor_core::{DiagnosticError, DiskIoRates, PlatformResult};

pub(super) fn cumulative_bytes() -> Result<(u64, u64), PlatformResult<DiskIoRates>> {
    let text = fs::read_to_string("/proc/diskstats").map_err(|error| {
        if error.kind() == io::ErrorKind::PermissionDenied {
            PlatformResult::PermissionRequired(format!("/proc/diskstats denied: {error}"))
        } else {
            PlatformResult::Error(DiagnosticError::PlatformError(format!(
                "failed to read /proc/diskstats: {error}"
            )))
        }
    })?;

    let mut read_sectors = 0_u64;
    let mut write_sectors = 0_u64;
    let mut devices = 0_u32;
    for line in text.lines() {
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() < 14 {
            continue;
        }
        let name = fields[2];
        if looks_like_partition(name) {
            continue;
        }
        let Ok(sectors_read) = fields[5].parse::<u64>() else {
            continue;
        };
        let Ok(sectors_written) = fields[9].parse::<u64>() else {
            continue;
        };
        read_sectors = read_sectors.saturating_add(sectors_read);
        write_sectors = write_sectors.saturating_add(sectors_written);
        devices = devices.saturating_add(1);
    }

    if devices == 0 {
        return Err(PlatformResult::Unavailable(
            "No whole-disk entries found in /proc/diskstats".into(),
        ));
    }
    Ok((
        read_sectors.saturating_mul(512),
        write_sectors.saturating_mul(512),
    ))
}

fn looks_like_partition(name: &str) -> bool {
    // Keep whole disks (sda, nvme0n1, mmcblk0); skip partitions (sda1, nvme0n1p2).
    if let Some(rest) = name.strip_prefix("nvme") {
        return rest.contains('p') && rest.chars().last().is_some_and(|c| c.is_ascii_digit());
    }
    if let Some(rest) = name.strip_prefix("mmcblk") {
        return rest.contains('p');
    }
    name.chars().last().is_some_and(|c| c.is_ascii_digit())
        && name.chars().any(|c| c.is_ascii_alphabetic())
}

#[cfg(test)]
mod tests {
    use super::looks_like_partition;

    #[test]
    fn partition_heuristic_keeps_disks_and_skips_slices() {
        assert!(!looks_like_partition("sda"));
        assert!(looks_like_partition("sda1"));
        assert!(!looks_like_partition("nvme0n1"));
        assert!(looks_like_partition("nvme0n1p2"));
        assert!(!looks_like_partition("mmcblk0"));
        assert!(looks_like_partition("mmcblk0p1"));
    }
}

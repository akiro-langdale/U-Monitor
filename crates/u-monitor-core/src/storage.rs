//! Mounted-volume information and optional live disk I/O rates.
//! Rates are CALCULATED from MEASURED cumulative counters; never simulated.
use crate::PlatformResult;
use std::{ffi::OsString, fmt, path::PathBuf, time::Duration};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageKind {
    Hdd,
    Ssd,
    Unknown(isize),
}

impl fmt::Display for StorageKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Hdd => f.write_str("HDD"),
            Self::Ssd => f.write_str("SSD"),
            Self::Unknown(code) => write!(f, "UNKNOWN ({code})"),
        }
    }
}

/// Backend-reported volume space in bytes, not raw device capacity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VolumeSpace {
    pub total_bytes: PlatformResult<u64>,
    pub available_bytes: PlatformResult<u64>,
    /// Calculated total - available; may include reservations and quota effects.
    /// Not necessarily the number of bytes allocated to files.
    pub used_bytes: PlatformResult<u64>,
}

/// One mounted filesystem reported by the backend. Do not sum these entries
/// to infer physical capacity: aliases and shared storage can overlap.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorageVolume {
    pub name: PlatformResult<OsString>,
    pub mount_point: PathBuf,
    pub file_system: PlatformResult<OsString>,
    pub kind: StorageKind,
    pub space: VolumeSpace,
}

/// Aggregate host disk throughput derived from cumulative device counters.
#[derive(Debug, Clone, PartialEq)]
pub struct DiskIoRates {
    /// Bytes read per second over the completed sample window.
    pub read_bytes_per_sec: PlatformResult<f64>,
    /// Bytes written per second over the same window.
    pub write_bytes_per_sec: PlatformResult<f64>,
    /// Observed interval between the two counter snapshots used for the rate.
    pub sample_interval: PlatformResult<Duration>,
}

/// Read-only request for a regular file hex sample. Never opens raw devices for write.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileReadRequest {
    pub path: PathBuf,
    pub offset: u64,
    pub length: usize,
}

/// Cap-limited bytes read from a user-selected regular file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileBytes {
    pub path: PathBuf,
    pub offset: u64,
    pub bytes: Vec<u8>,
}

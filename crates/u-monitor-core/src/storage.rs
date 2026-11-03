//! Mounted-volume information, not a physical-device inventory.
use crate::PlatformResult;
use std::{ffi::OsString, fmt, path::PathBuf};

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

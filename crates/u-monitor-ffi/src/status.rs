//! C-compatible status, provenance, and feature enumerations.

use std::os::raw::c_int;

/// Result code returned by every exported function.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UmStatus {
    Ok = 0,
    NullArgument = 1,
    BufferTooSmall = 2,
    Unavailable = 3,
    NotImplemented = 4,
    PermissionRequired = 5,
    PlatformError = 6,
    ValidationError = 7,
    Internal = 8,
}

impl UmStatus {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Ok => "OK",
            Self::NullArgument => "NULL_ARGUMENT",
            Self::BufferTooSmall => "BUFFER_TOO_SMALL",
            Self::Unavailable => "UNAVAILABLE",
            Self::NotImplemented => "NOT_IMPLEMENTED",
            Self::PermissionRequired => "PERMISSION_REQUIRED",
            Self::PlatformError => "PLATFORM_ERROR",
            Self::ValidationError => "VALIDATION_ERROR",
            Self::Internal => "INTERNAL",
        }
    }
}

/// Origin of a successful numeric or textual field.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UmDataSource {
    Measured = 1,
    Calculated = 2,
    Simulated = 3,
}

/// Availability of a platform capability on the current host.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UmAvailability {
    Supported = 1,
    PermissionRequired = 2,
    Unavailable = 3,
    NotImplemented = 4,
    Error = 5,
}

/// Platform features that can be queried through the C ABI.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UmFeature {
    SystemIdentity = 1,
    CpuDiscovery = 2,
    CpuMetrics = 3,
    RamMetrics = 4,
    VirtualMemoryMap = 5,
    MemoryBytes = 6,
    StorageDiscovery = 7,
    DiskStorage = 8,
    HardwareRegisters = 9,
    CurrentThreadRegisters = 10,
    CacheTopology = 11,
}

impl UmFeature {
    pub(crate) fn from_c(value: c_int) -> Option<Self> {
        match value {
            x if x == Self::SystemIdentity as c_int => Some(Self::SystemIdentity),
            x if x == Self::CpuDiscovery as c_int => Some(Self::CpuDiscovery),
            x if x == Self::CpuMetrics as c_int => Some(Self::CpuMetrics),
            x if x == Self::RamMetrics as c_int => Some(Self::RamMetrics),
            x if x == Self::VirtualMemoryMap as c_int => Some(Self::VirtualMemoryMap),
            x if x == Self::MemoryBytes as c_int => Some(Self::MemoryBytes),
            x if x == Self::StorageDiscovery as c_int => Some(Self::StorageDiscovery),
            x if x == Self::DiskStorage as c_int => Some(Self::DiskStorage),
            x if x == Self::HardwareRegisters as c_int => Some(Self::HardwareRegisters),
            x if x == Self::CurrentThreadRegisters as c_int => Some(Self::CurrentThreadRegisters),
            x if x == Self::CacheTopology as c_int => Some(Self::CacheTopology),
            _ => None,
        }
    }
}

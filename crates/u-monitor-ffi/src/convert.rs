//! Shared conversions between Rust contracts and C ABI POD types.

use crate::status::{UmAvailability, UmDataSource, UmFeature, UmStatus};
use std::os::raw::c_char;
use std::panic::{AssertUnwindSafe, catch_unwind};
use u_monitor_core::{Availability, DataSource, PlatformFeature, PlatformResult};

/// One numeric field that may be missing without inventing a zero.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UmFieldU64 {
    pub status: UmStatus,
    pub source: UmDataSource,
    pub value: u64,
    /// Unix epoch milliseconds (UTC). Valid when `status == Ok`.
    pub timestamp_unix_ms: i64,
}

impl Default for UmFieldU64 {
    fn default() -> Self {
        Self {
            status: UmStatus::Unavailable,
            source: UmDataSource::Measured,
            value: 0,
            timestamp_unix_ms: 0,
        }
    }
}

pub(crate) fn write_cstr(text: &str, buffer: *mut c_char, capacity: usize) -> UmStatus {
    if buffer.is_null() {
        return UmStatus::NullArgument;
    }
    if capacity == 0 {
        return UmStatus::BufferTooSmall;
    }
    let bytes = text.as_bytes();
    if bytes.len() + 1 > capacity {
        return UmStatus::BufferTooSmall;
    }
    unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), buffer.cast::<u8>(), bytes.len());
        *buffer.add(bytes.len()) = 0;
    }
    UmStatus::Ok
}

pub(crate) fn catch_status(f: impl FnOnce() -> UmStatus) -> UmStatus {
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(status) => status,
        Err(_) => UmStatus::Internal,
    }
}

pub(crate) fn data_source(source: DataSource) -> UmDataSource {
    match source {
        DataSource::Measured => UmDataSource::Measured,
        DataSource::Calculated => UmDataSource::Calculated,
        DataSource::Simulated => UmDataSource::Simulated,
    }
}

pub(crate) fn availability(value: Availability) -> UmAvailability {
    match value {
        Availability::Supported => UmAvailability::Supported,
        Availability::PermissionRequired => UmAvailability::PermissionRequired,
        Availability::Unavailable => UmAvailability::Unavailable,
        Availability::NotImplemented => UmAvailability::NotImplemented,
        Availability::Error => UmAvailability::Error,
    }
}

pub(crate) fn feature(value: UmFeature) -> PlatformFeature {
    match value {
        UmFeature::SystemIdentity => PlatformFeature::SystemIdentity,
        UmFeature::CpuDiscovery => PlatformFeature::CpuDiscovery,
        UmFeature::CpuMetrics => PlatformFeature::CpuMetrics,
        UmFeature::RamMetrics => PlatformFeature::RamMetrics,
        UmFeature::VirtualMemoryMap => PlatformFeature::VirtualMemoryMap,
        UmFeature::MemoryBytes => PlatformFeature::MemoryBytes,
        UmFeature::StorageDiscovery => PlatformFeature::StorageDiscovery,
        UmFeature::DiskStorage => PlatformFeature::DiskStorage,
        UmFeature::HardwareRegisters => PlatformFeature::HardwareRegisters,
        UmFeature::CurrentThreadRegisters => PlatformFeature::CurrentThreadRegisters,
        UmFeature::CacheTopology => PlatformFeature::CacheTopology,
    }
}

pub(crate) fn result_status<T>(result: &PlatformResult<T>) -> UmStatus {
    match result {
        PlatformResult::Success(_) => UmStatus::Ok,
        PlatformResult::PermissionRequired(_) => UmStatus::PermissionRequired,
        PlatformResult::Unavailable(_) => UmStatus::Unavailable,
        PlatformResult::NotImplemented(_) => UmStatus::NotImplemented,
        PlatformResult::Error(_) => UmStatus::PlatformError,
    }
}

pub(crate) fn field_u64(result: &PlatformResult<u64>) -> UmFieldU64 {
    match result {
        PlatformResult::Success(measurement) => UmFieldU64 {
            status: UmStatus::Ok,
            source: data_source(measurement.source),
            value: measurement.value,
            timestamp_unix_ms: measurement.timestamp.timestamp_millis(),
        },
        other => UmFieldU64 {
            status: result_status(other),
            source: UmDataSource::Measured,
            value: 0,
            timestamp_unix_ms: 0,
        },
    }
}

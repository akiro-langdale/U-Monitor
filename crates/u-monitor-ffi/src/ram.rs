//! RAM statistics through the C ABI. Never exposes physical memory bytes.

use crate::convert::{UmFieldU64, catch_status, field_u64, result_status};
use crate::status::UmStatus;
use u_monitor_core::PlatformAdapter;
use u_monitor_platform::NativePlatformAdapter;

/// OS-visible RAM counters. Missing fields keep a non-Ok status instead of zero.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct UmRamInfo {
    pub status: UmStatus,
    pub total_bytes: UmFieldU64,
    pub available_bytes: UmFieldU64,
    pub used_bytes: UmFieldU64,
}

impl Default for UmRamInfo {
    fn default() -> Self {
        Self {
            status: UmStatus::Unavailable,
            total_bytes: UmFieldU64::default(),
            available_bytes: UmFieldU64::default(),
            used_bytes: UmFieldU64::default(),
        }
    }
}

/// Fills `out` with a RAM snapshot. `used_bytes` is CALCULATED when both inputs exist.
#[unsafe(no_mangle)]
pub extern "C" fn um_ram_info(out: *mut UmRamInfo) -> UmStatus {
    catch_status(|| {
        if out.is_null() {
            return UmStatus::NullArgument;
        }
        let mut value = UmRamInfo::default();
        let result = NativePlatformAdapter.ram_info();
        value.status = result_status(&result);
        if let u_monitor_core::PlatformResult::Success(measurement) = result {
            value.total_bytes = field_u64(&measurement.value.total_bytes);
            value.available_bytes = field_u64(&measurement.value.available_bytes);
            value.used_bytes = field_u64(&measurement.value.used_bytes);
        }
        let status = value.status;
        unsafe {
            *out = value;
        }
        status
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::status::UmDataSource;

    #[test]
    fn ram_snapshot_preserves_field_provenance() {
        let mut info = UmRamInfo::default();
        let status = um_ram_info(&mut info);
        assert_eq!(status, UmStatus::Ok);
        assert_eq!(info.status, UmStatus::Ok);
        if info.total_bytes.status == UmStatus::Ok {
            assert_eq!(info.total_bytes.source, UmDataSource::Measured);
            assert!(info.total_bytes.value > 0);
        }
        if info.used_bytes.status == UmStatus::Ok {
            assert_eq!(info.used_bytes.source, UmDataSource::Calculated);
        }
    }

    #[test]
    fn null_ram_output_is_rejected() {
        assert_eq!(um_ram_info(std::ptr::null_mut()), UmStatus::NullArgument);
    }
}

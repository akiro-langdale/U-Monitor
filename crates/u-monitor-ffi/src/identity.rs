//! System identity and capability queries exposed through the C ABI.

use crate::convert::{self, availability, catch_status, data_source, feature, result_status};
use crate::status::{UmAvailability, UmDataSource, UmFeature, UmStatus};
use std::os::raw::{c_char, c_int};
use u_monitor_core::PlatformAdapter;
use u_monitor_platform::NativePlatformAdapter;

const LABEL_CAP: usize = 64;

/// Build-target identity. Buffers are UTF-8 and NUL-terminated when status is Ok.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct UmSystemIdentity {
    pub status: UmStatus,
    pub source: UmDataSource,
    pub timestamp_unix_ms: i64,
    pub operating_system: [c_char; LABEL_CAP],
    pub process_architecture: [c_char; LABEL_CAP],
}

impl Default for UmSystemIdentity {
    fn default() -> Self {
        Self {
            status: UmStatus::Unavailable,
            source: UmDataSource::Measured,
            timestamp_unix_ms: 0,
            operating_system: [0; LABEL_CAP],
            process_architecture: [0; LABEL_CAP],
        }
    }
}

/// Fills `out` with MEASURED build-target metadata for the running binary.
#[unsafe(no_mangle)]
pub extern "C" fn um_system_identity(out: *mut UmSystemIdentity) -> UmStatus {
    catch_status(|| {
        if out.is_null() {
            return UmStatus::NullArgument;
        }
        let mut value = UmSystemIdentity::default();
        let result = NativePlatformAdapter.system_identity();
        value.status = result_status(&result);
        if let u_monitor_core::PlatformResult::Success(measurement) = result {
            value.source = data_source(measurement.source);
            value.timestamp_unix_ms = measurement.timestamp.timestamp_millis();
            let os = measurement.value.operating_system.to_string();
            let arch = measurement.value.process_architecture.to_string();
            if convert::write_cstr(&os, value.operating_system.as_mut_ptr(), LABEL_CAP)
                != UmStatus::Ok
                || convert::write_cstr(&arch, value.process_architecture.as_mut_ptr(), LABEL_CAP)
                    != UmStatus::Ok
            {
                value.status = UmStatus::BufferTooSmall;
                unsafe {
                    *out = value;
                }
                return UmStatus::BufferTooSmall;
            }
        }
        let status = value.status;
        unsafe {
            *out = value;
        }
        status
    })
}

/// Writes the availability of `feature` into `out`.
#[unsafe(no_mangle)]
pub extern "C" fn um_check_availability(feature_code: c_int, out: *mut UmAvailability) -> UmStatus {
    catch_status(|| {
        if out.is_null() {
            return UmStatus::NullArgument;
        }
        let Some(selected) = UmFeature::from_c(feature_code) else {
            return UmStatus::ValidationError;
        };
        let value = availability(NativePlatformAdapter.check_availability(feature(selected)));
        unsafe {
            *out = value;
        }
        UmStatus::Ok
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_identity_is_measured() {
        let mut identity = UmSystemIdentity::default();
        assert_eq!(um_system_identity(&mut identity), UmStatus::Ok);
        assert_eq!(identity.status, UmStatus::Ok);
        assert_eq!(identity.source, UmDataSource::Measured);
        assert_ne!(identity.operating_system[0], 0);
        assert_ne!(identity.process_architecture[0], 0);
    }

    #[test]
    fn null_outputs_are_rejected() {
        assert_eq!(
            um_system_identity(std::ptr::null_mut()),
            UmStatus::NullArgument
        );
        assert_eq!(
            um_check_availability(UmFeature::SystemIdentity as c_int, std::ptr::null_mut()),
            UmStatus::NullArgument
        );
    }
}

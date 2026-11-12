//! Bounded read-only samples from the current process virtual address space.

use crate::convert::{catch_status, data_source, result_status};
use crate::status::{UmDataSource, UmStatus};
use u_monitor_core::{MemoryReadRequest, PlatformAdapter};
use u_monitor_platform::{MAX_MEMORY_SAMPLE_BYTES, NativePlatformAdapter};

/// Metadata for a memory byte sample written into a caller-owned buffer.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct UmMemorySampleInfo {
    pub status: UmStatus,
    pub source: UmDataSource,
    pub address: u64,
    pub bytes_read: usize,
    pub timestamp_unix_ms: i64,
}

impl Default for UmMemorySampleInfo {
    fn default() -> Self {
        Self {
            status: UmStatus::Unavailable,
            source: UmDataSource::Measured,
            address: 0,
            bytes_read: 0,
            timestamp_unix_ms: 0,
        }
    }
}

/// Reads up to `length` bytes from `address` in the current process.
///
/// `length` and `capacity` are capped at [`MAX_MEMORY_SAMPLE_BYTES`]. Returns
/// [`UmStatus::ValidationError`] when `length == 0` or the range overflows.
#[unsafe(no_mangle)]
pub extern "C" fn um_memory_sample(
    address: u64,
    length: usize,
    buffer: *mut u8,
    capacity: usize,
    out: *mut UmMemorySampleInfo,
) -> UmStatus {
    catch_status(|| {
        if out.is_null() {
            return UmStatus::NullArgument;
        }
        if buffer.is_null() {
            return UmStatus::NullArgument;
        }
        if length == 0 {
            return UmStatus::ValidationError;
        }
        let max_len = length.min(capacity).min(MAX_MEMORY_SAMPLE_BYTES);
        if max_len == 0 {
            return UmStatus::BufferTooSmall;
        }
        let mut info = UmMemorySampleInfo {
            address,
            ..UmMemorySampleInfo::default()
        };
        let result = NativePlatformAdapter.read_current_process_bytes(MemoryReadRequest {
            address,
            length: max_len,
        });
        info.status = result_status(&result);
        if let u_monitor_core::PlatformResult::Success(measurement) = result {
            info.source = data_source(measurement.source);
            info.timestamp_unix_ms = measurement.timestamp.timestamp_millis();
            info.bytes_read = measurement.value.bytes.len();
            if info.bytes_read > capacity {
                unsafe {
                    *out = info;
                }
                return UmStatus::BufferTooSmall;
            }
            unsafe {
                std::ptr::copy_nonoverlapping(
                    measurement.value.bytes.as_ptr(),
                    buffer,
                    info.bytes_read,
                );
            }
        }
        let status = info.status;
        unsafe {
            *out = info;
        }
        status
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn null_outputs_are_rejected() {
        let mut info = UmMemorySampleInfo::default();
        assert_eq!(
            um_memory_sample(0, 8, std::ptr::null_mut(), 8, &mut info),
            UmStatus::NullArgument
        );
        let mut buf = [0u8; 8];
        assert_eq!(
            um_memory_sample(0, 8, buf.as_mut_ptr(), 8, std::ptr::null_mut()),
            UmStatus::NullArgument
        );
    }

    #[test]
    fn zero_length_is_validation_error() {
        let mut info = UmMemorySampleInfo::default();
        let mut buf = [0u8; 8];
        assert_eq!(
            um_memory_sample(0, 0, buf.as_mut_ptr(), 8, &mut info),
            UmStatus::ValidationError
        );
    }
}

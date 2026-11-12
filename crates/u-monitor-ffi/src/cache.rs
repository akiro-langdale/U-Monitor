//! CPU cache topology exposed through the C ABI.

use crate::convert::{
    UmFieldU32, catch_status, data_source, field_u32_optional, optional_measured_u64, result_status,
};
use crate::status::{UmDataSource, UmStatus};
use u_monitor_core::{CacheKind, PlatformAdapter};
use u_monitor_platform::NativePlatformAdapter;

/// Contents held by a cache level, as reported by the OS.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UmCacheKind {
    Data = 1,
    Instruction = 2,
    Unified = 3,
    Trace = 4,
    Unknown = 5,
}

fn cache_kind(value: CacheKind) -> UmCacheKind {
    match value {
        CacheKind::Data => UmCacheKind::Data,
        CacheKind::Instruction => UmCacheKind::Instruction,
        CacheKind::Unified => UmCacheKind::Unified,
        CacheKind::Trace => UmCacheKind::Trace,
        CacheKind::Unknown => UmCacheKind::Unknown,
    }
}

/// One OS-reported cache node. Optional metrics stay unavailable instead of zero.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct UmCacheNode {
    pub level: u32,
    pub kind: UmCacheKind,
    pub size_bytes: crate::convert::UmFieldU64,
    pub line_size_bytes: UmFieldU32,
    pub associativity: UmFieldU32,
    /// Non-zero when the OS marks the cache as fully associative.
    pub fully_associative: u8,
}

/// Snapshot header for cache topology. `node_count` is valid when `status == Ok`.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct UmCacheTopologyHeader {
    pub status: UmStatus,
    pub source: UmDataSource,
    pub timestamp_unix_ms: i64,
    pub node_count: usize,
}

impl Default for UmCacheTopologyHeader {
    fn default() -> Self {
        Self {
            status: UmStatus::Unavailable,
            source: UmDataSource::Measured,
            timestamp_unix_ms: 0,
            node_count: 0,
        }
    }
}

/// Fills `header` and optionally copies up to `capacity` nodes into `nodes`.
///
/// When `nodes` is null or `capacity == 0`, only the header (including `node_count`) is written.
/// Returns [`UmStatus::BufferTooSmall`] when `node_count > capacity` and `nodes` is non-null.
#[unsafe(no_mangle)]
pub extern "C" fn um_cache_topology(
    header: *mut UmCacheTopologyHeader,
    nodes: *mut UmCacheNode,
    capacity: usize,
) -> UmStatus {
    catch_status(|| {
        if header.is_null() {
            return UmStatus::NullArgument;
        }
        let mut hdr = UmCacheTopologyHeader::default();
        let result = NativePlatformAdapter.cache_topology();
        hdr.status = result_status(&result);
        if let u_monitor_core::PlatformResult::Success(measurement) = result {
            hdr.source = data_source(measurement.source);
            hdr.timestamp_unix_ms = measurement.timestamp.timestamp_millis();
            hdr.node_count = measurement.value.nodes.len();
            if !nodes.is_null() && capacity > 0 {
                if hdr.node_count > capacity {
                    unsafe {
                        *header = hdr;
                    }
                    return UmStatus::BufferTooSmall;
                }
                for (index, node) in measurement.value.nodes.iter().enumerate() {
                    let ts = hdr.timestamp_unix_ms;
                    let written = UmCacheNode {
                        level: node.level,
                        kind: cache_kind(node.kind),
                        size_bytes: optional_measured_u64(node.size_bytes, ts),
                        line_size_bytes: field_u32_optional(node.line_size_bytes, ts),
                        associativity: field_u32_optional(node.associativity, ts),
                        fully_associative: u8::from(node.fully_associative),
                    };
                    unsafe {
                        *nodes.add(index) = written;
                    }
                }
            }
        }
        let status = hdr.status;
        unsafe {
            *header = hdr;
        }
        status
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::status::UmDataSource;

    #[test]
    fn cache_topology_header_is_measured() {
        let mut header = UmCacheTopologyHeader::default();
        assert_eq!(
            um_cache_topology(&mut header, std::ptr::null_mut(), 0),
            UmStatus::Ok
        );
        assert_eq!(header.status, UmStatus::Ok);
        assert_eq!(header.source, UmDataSource::Measured);
    }

    #[test]
    fn null_header_is_rejected() {
        assert_eq!(
            um_cache_topology(std::ptr::null_mut(), std::ptr::null_mut(), 0),
            UmStatus::NullArgument
        );
    }

    #[test]
    fn undersized_node_buffer_reports_too_small() {
        let mut header = UmCacheTopologyHeader::default();
        if um_cache_topology(&mut header, std::ptr::null_mut(), 0) != UmStatus::Ok
            || header.node_count < 2
        {
            return;
        }
        let mut nodes = vec![
            UmCacheNode {
                level: 0,
                kind: UmCacheKind::Unknown,
                size_bytes: crate::convert::UmFieldU64::default(),
                line_size_bytes: UmFieldU32::default(),
                associativity: UmFieldU32::default(),
                fully_associative: 0,
            };
            1
        ];
        assert_eq!(
            um_cache_topology(&mut header, nodes.as_mut_ptr(), 1),
            UmStatus::BufferTooSmall
        );
    }
}

//! SIMULATED memory-hierarchy handle for educational cache modelling.

use crate::convert::{catch_status, data_source};
use crate::status::{UmDataSource, UmStatus};
use std::panic::{AssertUnwindSafe, catch_unwind};
use u_monitor_simulation::{HierarchySimulator, MemoryLevel};

/// Opaque simulator owned by the library until `um_hierarchy_destroy`.
pub struct UmHierarchy {
    inner: HierarchySimulator,
}

/// One modelled access. Always carries `UM_SOURCE_SIMULATED`.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UmAccessResult {
    pub status: UmStatus,
    pub source: UmDataSource,
    pub address: u64,
    pub line_address: u64,
    /// 1 = L1, 2 = L2, 3 = RAM.
    pub resolved_level: u32,
    pub hit: u8,
    pub latency_ns: u64,
    pub step_count: u32,
}

/// Aggregate simulator counters. Always SIMULATED.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UmHierarchyStats {
    pub status: UmStatus,
    pub source: UmDataSource,
    pub accesses: u64,
    pub l1_hits: u64,
    pub l2_hits: u64,
    pub ram_fills: u64,
    pub evictions: u64,
    pub total_latency_ns: u64,
}

impl Default for UmHierarchyStats {
    fn default() -> Self {
        Self {
            status: UmStatus::Unavailable,
            source: UmDataSource::Simulated,
            accesses: 0,
            l1_hits: 0,
            l2_hits: 0,
            ram_fills: 0,
            evictions: 0,
            total_latency_ns: 0,
        }
    }
}

fn level_code(level: MemoryLevel) -> u32 {
    match level {
        MemoryLevel::L1 => 1,
        MemoryLevel::L2 => 2,
        MemoryLevel::Ram => 3,
    }
}

/// Creates a default L1/L2/RAM simulator. Returns null on allocation failure.
#[unsafe(no_mangle)]
pub extern "C" fn um_hierarchy_create() -> *mut UmHierarchy {
    match catch_unwind(HierarchySimulator::new_default) {
        Ok(inner) => Box::into_raw(Box::new(UmHierarchy { inner })),
        Err(_) => std::ptr::null_mut(),
    }
}

/// Releases a hierarchy created by [`um_hierarchy_create`]. Null is a no-op.
#[unsafe(no_mangle)]
pub extern "C" fn um_hierarchy_destroy(handle: *mut UmHierarchy) {
    if handle.is_null() {
        return;
    }
    let _ = catch_unwind(AssertUnwindSafe(|| unsafe {
        drop(Box::from_raw(handle));
    }));
}

/// Clears cache contents and counters without destroying the handle.
#[unsafe(no_mangle)]
pub extern "C" fn um_hierarchy_reset(handle: *mut UmHierarchy) -> UmStatus {
    catch_status(|| {
        let Some(hierarchy) = (unsafe { handle.as_mut() }) else {
            return UmStatus::NullArgument;
        };
        hierarchy.inner.reset();
        UmStatus::Ok
    })
}

/// Models one load at `address`. Outcome and latency are SIMULATED.
#[unsafe(no_mangle)]
pub extern "C" fn um_hierarchy_access(
    handle: *mut UmHierarchy,
    address: u64,
    out: *mut UmAccessResult,
) -> UmStatus {
    catch_status(|| {
        if out.is_null() {
            return UmStatus::NullArgument;
        }
        let Some(hierarchy) = (unsafe { handle.as_mut() }) else {
            return UmStatus::NullArgument;
        };
        let measurement = hierarchy.inner.access(address);
        let value = UmAccessResult {
            status: UmStatus::Ok,
            source: data_source(measurement.source),
            address: measurement.value.address,
            line_address: measurement.value.line_address,
            resolved_level: level_code(measurement.value.resolved_at),
            hit: u8::from(measurement.value.hit),
            latency_ns: measurement.value.latency_ns,
            step_count: measurement.value.steps.len() as u32,
        };
        unsafe {
            *out = value;
        }
        UmStatus::Ok
    })
}

/// Copies SIMULATED aggregate counters into `out`.
#[unsafe(no_mangle)]
pub extern "C" fn um_hierarchy_stats(
    handle: *const UmHierarchy,
    out: *mut UmHierarchyStats,
) -> UmStatus {
    catch_status(|| {
        if out.is_null() {
            return UmStatus::NullArgument;
        }
        let Some(hierarchy) = (unsafe { handle.as_ref() }) else {
            return UmStatus::NullArgument;
        };
        let measurement = hierarchy.inner.stats();
        let value = UmHierarchyStats {
            status: UmStatus::Ok,
            source: data_source(measurement.source),
            accesses: measurement.value.accesses,
            l1_hits: measurement.value.l1_hits,
            l2_hits: measurement.value.l2_hits,
            ram_fills: measurement.value.ram_fills,
            evictions: measurement.value.evictions,
            total_latency_ns: measurement.value.total_latency_ns,
        };
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
    fn hierarchy_marks_simulated_hit_after_fill() {
        let handle = um_hierarchy_create();
        assert!(!handle.is_null());
        let mut first = UmAccessResult {
            status: UmStatus::Internal,
            source: UmDataSource::Measured,
            address: 0,
            line_address: 0,
            resolved_level: 0,
            hit: 0,
            latency_ns: 0,
            step_count: 0,
        };
        assert_eq!(
            um_hierarchy_access(handle, 0x1000, &mut first),
            UmStatus::Ok
        );
        assert_eq!(first.source, UmDataSource::Simulated);
        assert_eq!(first.hit, 0);
        assert_eq!(first.resolved_level, 3);

        let mut second = first;
        assert_eq!(
            um_hierarchy_access(handle, 0x1000, &mut second),
            UmStatus::Ok
        );
        assert_eq!(second.hit, 1);
        assert_eq!(second.resolved_level, 1);
        assert_eq!(second.source, UmDataSource::Simulated);

        let mut stats = UmHierarchyStats::default();
        assert_eq!(um_hierarchy_stats(handle, &mut stats), UmStatus::Ok);
        assert_eq!(stats.source, UmDataSource::Simulated);
        assert_eq!(stats.accesses, 2);
        assert_eq!(stats.l1_hits, 1);
        assert_eq!(stats.ram_fills, 1);

        um_hierarchy_destroy(handle);
    }
}

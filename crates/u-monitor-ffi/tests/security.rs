//! FFI security / ownership smoke checks for the public C ABI.

use std::os::raw::c_char;
use u_monitor::{
    UmAccessResult, UmDataSource, UmHierarchyStats, UmMemorySampleInfo, UmRamInfo, UmStatus,
    UmSystemIdentity, um_cache_topology, um_hierarchy_access, um_hierarchy_create,
    um_hierarchy_destroy, um_hierarchy_reset, um_hierarchy_stats, um_memory_sample, um_ram_info,
    um_status_name, um_system_identity,
};

#[test]
fn null_arguments_do_not_crash_and_report_null() {
    assert_eq!(
        um_system_identity(std::ptr::null_mut()),
        UmStatus::NullArgument
    );
    assert_eq!(um_ram_info(std::ptr::null_mut()), UmStatus::NullArgument);
    assert_eq!(
        um_cache_topology(std::ptr::null_mut(), std::ptr::null_mut(), 0),
        UmStatus::NullArgument
    );
    let mut sample = UmMemorySampleInfo::default();
    assert_eq!(
        um_memory_sample(0, 1, std::ptr::null_mut(), 1, &mut sample),
        UmStatus::NullArgument
    );
    let mut byte = 0u8;
    assert_eq!(
        um_memory_sample(0, 1, &mut byte, 1, std::ptr::null_mut()),
        UmStatus::NullArgument
    );
    assert_eq!(
        um_status_name(UmStatus::Ok, std::ptr::null_mut(), 8),
        UmStatus::NullArgument
    );
    assert_eq!(
        um_hierarchy_access(std::ptr::null_mut(), 0, std::ptr::null_mut()),
        UmStatus::NullArgument
    );
    assert_eq!(
        um_hierarchy_stats(std::ptr::null(), std::ptr::null_mut()),
        UmStatus::NullArgument
    );
    assert_eq!(
        um_hierarchy_reset(std::ptr::null_mut()),
        UmStatus::NullArgument
    );
    um_hierarchy_destroy(std::ptr::null_mut());
}

#[test]
fn tiny_status_buffer_is_rejected_without_overflow() {
    let mut tiny = [0 as c_char; 2];
    assert_eq!(
        um_status_name(UmStatus::NotImplemented, tiny.as_mut_ptr(), tiny.len()),
        UmStatus::BufferTooSmall
    );
    assert_eq!(tiny[0], 0);
}

#[test]
fn hierarchy_destroy_after_use_is_safe() {
    let handle = um_hierarchy_create();
    assert!(!handle.is_null());
    let mut access = UmAccessResult {
        status: UmStatus::Internal,
        source: UmDataSource::Measured,
        address: 0,
        line_address: 0,
        resolved_level: 0,
        hit: 0,
        latency_ns: 0,
        step_count: 0,
    };
    assert_eq!(um_hierarchy_access(handle, 0x40, &mut access), UmStatus::Ok);
    assert_eq!(access.source, UmDataSource::Simulated);
    let mut stats = UmHierarchyStats::default();
    assert_eq!(um_hierarchy_stats(handle, &mut stats), UmStatus::Ok);
    um_hierarchy_destroy(handle);
}

#[test]
fn identity_and_ram_never_claim_simulated() {
    let mut identity = UmSystemIdentity::default();
    assert_eq!(um_system_identity(&mut identity), UmStatus::Ok);
    assert_ne!(identity.source, UmDataSource::Simulated);

    let mut ram = UmRamInfo::default();
    assert_eq!(um_ram_info(&mut ram), UmStatus::Ok);
    if ram.total_bytes.status == UmStatus::Ok {
        assert_ne!(ram.total_bytes.source, UmDataSource::Simulated);
    }
}

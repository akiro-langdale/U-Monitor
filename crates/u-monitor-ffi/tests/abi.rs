use std::os::raw::c_int;
use u_monitor::{
    UmAccessResult, UmAvailability, UmDataSource, UmFeature, UmHierarchyStats, UmRamInfo, UmStatus,
    UmSystemIdentity, um_check_availability, um_hierarchy_access, um_hierarchy_create,
    um_hierarchy_destroy, um_hierarchy_stats, um_ram_info, um_status_name, um_system_identity,
    um_version,
};

#[test]
fn public_abi_smoke_preserves_provenance() {
    let version = unsafe { std::ffi::CStr::from_ptr(um_version()) };
    assert_eq!(version.to_str().unwrap(), "0.1.0");

    let mut name = [0 as std::os::raw::c_char; 32];
    assert_eq!(
        um_status_name(UmStatus::Ok, name.as_mut_ptr(), name.len()),
        UmStatus::Ok
    );

    let mut identity = UmSystemIdentity::default();
    assert_eq!(um_system_identity(&mut identity), UmStatus::Ok);
    assert_eq!(identity.source, UmDataSource::Measured);

    let mut availability = UmAvailability::Error;
    assert_eq!(
        um_check_availability(UmFeature::SystemIdentity as c_int, &mut availability),
        UmStatus::Ok
    );
    assert_eq!(availability, UmAvailability::Supported);

    let mut ram = UmRamInfo::default();
    assert_eq!(um_ram_info(&mut ram), UmStatus::Ok);

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
    assert_eq!(
        um_hierarchy_access(handle, 0x2000, &mut access),
        UmStatus::Ok
    );
    assert_eq!(access.source, UmDataSource::Simulated);
    let mut stats = UmHierarchyStats::default();
    assert_eq!(um_hierarchy_stats(handle, &mut stats), UmStatus::Ok);
    assert_eq!(stats.source, UmDataSource::Simulated);
    um_hierarchy_destroy(handle);
}

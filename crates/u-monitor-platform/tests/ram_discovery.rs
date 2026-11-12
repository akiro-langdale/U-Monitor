use u_monitor_core::{Availability, DataSource, PlatformAdapter, PlatformFeature, PlatformResult};
use u_monitor_platform::{NativePlatformAdapter, StubPlatformAdapter};

#[test]
fn stub_ram_request_does_not_return_fake_data() {
    let adapter = StubPlatformAdapter::new();
    assert_eq!(
        adapter.check_availability(PlatformFeature::RamMetrics),
        Availability::NotImplemented
    );
    assert!(matches!(
        adapter.ram_info(),
        PlatformResult::NotImplemented(_)
    ));
}

#[test]
#[ignore = "reads real host RAM; run explicitly on a target environment"]
fn native_ram_snapshot_has_valid_units_provenance_and_arithmetic() {
    if !sysinfo::IS_SUPPORTED_SYSTEM {
        assert!(matches!(
            NativePlatformAdapter.ram_info(),
            PlatformResult::Unavailable(_)
        ));
        return;
    }
    assert_eq!(
        NativePlatformAdapter.check_availability(PlatformFeature::RamMetrics),
        Availability::Supported
    );
    let PlatformResult::Success(snapshot) = NativePlatformAdapter.ram_info() else {
        panic!("target acceptance requires actual RAM data");
    };
    let PlatformResult::Success(total) = snapshot.value.total_bytes else {
        panic!("total missing")
    };
    let PlatformResult::Success(available) = snapshot.value.available_bytes else {
        panic!("available missing")
    };
    let PlatformResult::Success(used) = snapshot.value.used_bytes else {
        panic!("used missing")
    };
    assert!(total.value > 0);
    assert!(available.value <= total.value);
    assert_eq!(used.value, total.value - available.value);
    assert_eq!(total.source, DataSource::Measured);
    assert_eq!(available.source, DataSource::Measured);
    assert_eq!(used.source, DataSource::Calculated);
    assert_eq!(total.timestamp, available.timestamp);
    assert_eq!(total.timestamp, used.timestamp);
}

use u_monitor_core::{
    Availability, DataSource, PlatformAdapter, PlatformFeature, PlatformResult, StorageKind,
};
use u_monitor_platform::{NativePlatformAdapter, StubPlatformAdapter};

#[test]
fn stub_does_not_invent_volumes() {
    let adapter = StubPlatformAdapter::new();
    assert_eq!(
        adapter.check_availability(PlatformFeature::StorageDiscovery),
        Availability::NotImplemented
    );
    assert!(matches!(
        adapter.storage_volumes(),
        PlatformResult::NotImplemented(_)
    ));
}

#[test]
fn unknown_media_is_explicit_and_preserves_backend_code() {
    assert_eq!(StorageKind::Unknown(-1).to_string(), "UNKNOWN (-1)");
    assert_eq!(StorageKind::Ssd.to_string(), "SSD");
    assert_eq!(StorageKind::Hdd.to_string(), "HDD");
}

#[test]
#[ignore = "enumerates real mounted volumes; run explicitly on target environment"]
fn native_discovery_returns_actual_volumes_and_consistent_space() {
    if !sysinfo::IS_SUPPORTED_SYSTEM {
        assert!(matches!(
            NativePlatformAdapter.storage_volumes(),
            PlatformResult::Unavailable(_)
        ));
        return;
    }
    assert_eq!(
        NativePlatformAdapter.check_availability(PlatformFeature::StorageDiscovery),
        Availability::Supported
    );
    let PlatformResult::Success(snapshot) = NativePlatformAdapter.storage_volumes() else {
        panic!("target acceptance requires real mounted volumes");
    };
    assert_eq!(snapshot.source, DataSource::Measured);
    assert!(!snapshot.value.is_empty());
    let mut measured_space = false;
    for volume in snapshot.value {
        assert!(volume.mount_point.is_absolute());
        if let (
            PlatformResult::Success(total),
            PlatformResult::Success(available),
            PlatformResult::Success(used),
        ) = (
            volume.space.total_bytes,
            volume.space.available_bytes,
            volume.space.used_bytes,
        ) {
            assert!(total.value > 0);
            assert!(available.value <= total.value);
            assert_eq!(used.value, total.value - available.value);
            assert_eq!(total.source, DataSource::Measured);
            assert_eq!(available.source, DataSource::Measured);
            assert_eq!(used.source, DataSource::Calculated);
            assert_eq!(total.timestamp, used.timestamp);
            measured_space = true;
        }
    }
    assert!(
        measured_space,
        "target acceptance requires at least one space measurement"
    );
}

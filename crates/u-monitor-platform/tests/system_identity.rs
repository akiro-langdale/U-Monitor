use std::time::SystemTime;
use u_monitor_core::{
    Architecture, Availability, DataSource, OperatingSystem, PlatformAdapter, PlatformFeature,
    PlatformResult,
};
use u_monitor_platform::{NativePlatformAdapter, StubPlatformAdapter};

#[test]
fn native_identity_reads_target_metadata_and_acquisition_time() {
    let adapter: &dyn PlatformAdapter = &NativePlatformAdapter;
    let before = SystemTime::now();
    let result = adapter.system_identity();
    let after = SystemTime::now();
    assert_eq!(
        adapter.check_availability(PlatformFeature::SystemIdentity),
        Availability::Supported
    );
    let PlatformResult::Success(measurement) = result else {
        panic!("target metadata should be available");
    };
    assert_eq!(
        measurement.value.operating_system,
        OperatingSystem::from(std::env::consts::OS)
    );
    assert_eq!(
        measurement.value.process_architecture,
        Architecture::from(std::env::consts::ARCH)
    );
    assert_eq!(measurement.source, DataSource::Measured);
    // Wall clock may move backwards during acquisition; do not impose ordering then.
    if before <= after {
        assert!(measurement.timestamp >= before.into());
        assert!(measurement.timestamp <= after.into());
    }
}

#[test]
fn hardware_capabilities_remain_unimplemented() {
    let adapter = NativePlatformAdapter;
    for feature in [
        PlatformFeature::CpuMetrics,
        PlatformFeature::HardwareRegisters,
    ] {
        assert_eq!(
            adapter.check_availability(feature),
            Availability::NotImplemented
        );
    }
}

#[test]
fn stub_identity_does_not_return_fake_values() {
    let adapter = StubPlatformAdapter::new();
    assert_eq!(
        adapter.check_availability(PlatformFeature::SystemIdentity),
        Availability::NotImplemented
    );
    assert!(matches!(
        adapter.system_identity(),
        PlatformResult::NotImplemented(_)
    ));
}

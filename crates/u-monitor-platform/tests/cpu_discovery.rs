use u_monitor_core::{Availability, PlatformAdapter, PlatformFeature, PlatformResult};
use u_monitor_platform::{NativePlatformAdapter, StubPlatformAdapter};

#[test]
fn stub_cpu_discovery_returns_no_measurement() {
    let adapter = StubPlatformAdapter::new();
    assert_eq!(
        adapter.check_availability(PlatformFeature::CpuDiscovery),
        Availability::NotImplemented
    );
    assert!(matches!(
        adapter.cpu_info(),
        PlatformResult::NotImplemented(_)
    ));
}

#[test]
#[ignore = "reads real host CPU topology; run explicitly on a target environment"]
fn native_cpu_discovery_returns_real_data_or_explicit_unavailability() {
    let result = NativePlatformAdapter.cpu_info();
    match result {
        PlatformResult::Success(snapshot) => {
            assert_eq!(snapshot.source, u_monitor_core::DataSource::Measured);
            assert!(
                matches!(snapshot.value.model_names, PlatformResult::Success(_))
                    || matches!(
                        snapshot.value.logical_processors,
                        PlatformResult::Success(_)
                    )
                    || matches!(snapshot.value.physical_cores, PlatformResult::Success(_))
            );
        }
        PlatformResult::Unavailable(reason) => assert!(!reason.is_empty()),
        other => panic!("unexpected result: {other:?}"),
    }
}

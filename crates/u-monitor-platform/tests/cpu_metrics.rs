use u_monitor_core::{Availability, PlatformAdapter, PlatformFeature, PlatformResult};
use u_monitor_platform::{NativePlatformAdapter, NativeTelemetryAdapter, StubPlatformAdapter};

#[test]
fn discovery_and_stub_adapters_do_not_invent_cpu_utilization() {
    for adapter in [
        &mut NativePlatformAdapter as &mut dyn PlatformAdapter,
        &mut StubPlatformAdapter,
    ] {
        assert_eq!(
            adapter.check_availability(PlatformFeature::CpuMetrics),
            Availability::NotImplemented
        );
        assert!(matches!(
            adapter.cpu_metrics(),
            PlatformResult::NotImplemented(_)
        ));
    }
}

#[test]
fn stateful_adapter_first_call_is_not_a_zero_measurement() {
    let mut adapter = NativeTelemetryAdapter::default();
    // Unsupported platforms and a missing CPU list also return Unavailable.
    assert!(matches!(
        adapter.cpu_metrics(),
        PlatformResult::Unavailable(_)
    ));
    assert_eq!(
        adapter.check_availability(PlatformFeature::CpuMetrics),
        if sysinfo::IS_SUPPORTED_SYSTEM {
            Availability::Supported
        } else {
            Availability::Unavailable
        }
    );
}

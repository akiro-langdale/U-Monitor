use u_monitor_core::{Availability, PlatformAdapter, PlatformFeature};
use u_monitor_platform::StubPlatformAdapter;

#[test]
fn stub_reports_no_implemented_hardware_features() {
    let stub = StubPlatformAdapter::new();
    let adapter: &dyn PlatformAdapter = &stub;
    assert_eq!(adapter.name(), "Stub Platform Adapter");
    for feature in [
        PlatformFeature::CpuMetrics,
        PlatformFeature::RamMetrics,
        PlatformFeature::VirtualMemoryMap,
        PlatformFeature::MemoryBytes,
        PlatformFeature::DiskStorage,
        PlatformFeature::HardwareRegisters,
    ] {
        assert_eq!(
            adapter.check_availability(feature),
            Availability::NotImplemented
        );
    }
}

use u_monitor_core::{Availability, PlatformAdapter, PlatformFeature};
use u_monitor_platform::NativePlatformAdapter;

#[test]
fn native_capability_matrix_matches_host_limits() {
    let adapter = NativePlatformAdapter;
    assert_eq!(
        adapter.check_availability(PlatformFeature::SystemIdentity),
        Availability::Supported
    );

    for feature in [
        PlatformFeature::CpuMetrics,
        PlatformFeature::DiskStorage,
        PlatformFeature::HardwareRegisters,
    ] {
        assert_eq!(
            adapter.check_availability(feature),
            Availability::NotImplemented,
            "{feature}"
        );
    }

    #[cfg(any(windows, target_os = "linux"))]
    {
        assert_eq!(
            adapter.check_availability(PlatformFeature::VirtualMemoryMap),
            Availability::Supported
        );
        assert_eq!(
            adapter.check_availability(PlatformFeature::MemoryBytes),
            Availability::Supported
        );
        assert_eq!(
            adapter.check_availability(PlatformFeature::CacheTopology),
            Availability::Supported
        );
    }

    #[cfg(not(any(windows, target_os = "linux")))]
    {
        assert_eq!(
            adapter.check_availability(PlatformFeature::VirtualMemoryMap),
            Availability::NotImplemented
        );
        assert_eq!(
            adapter.check_availability(PlatformFeature::MemoryBytes),
            Availability::NotImplemented
        );
        assert_eq!(
            adapter.check_availability(PlatformFeature::CacheTopology),
            Availability::NotImplemented
        );
    }

    #[cfg(all(windows, target_arch = "x86_64"))]
    assert_eq!(
        adapter.check_availability(PlatformFeature::CurrentThreadRegisters),
        Availability::Supported
    );
    #[cfg(not(all(windows, target_arch = "x86_64")))]
    assert_eq!(
        adapter.check_availability(PlatformFeature::CurrentThreadRegisters),
        Availability::NotImplemented
    );
}

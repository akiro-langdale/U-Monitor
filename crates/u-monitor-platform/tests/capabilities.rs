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
        PlatformFeature::HardwareRegisters,
    ] {
        assert_eq!(
            adapter.check_availability(feature),
            Availability::NotImplemented,
            "{feature}"
        );
    }

    #[cfg(any(windows, target_os = "linux", target_os = "macos"))]
    assert_eq!(
        adapter.check_availability(PlatformFeature::DiskStorage),
        Availability::Supported
    );
    #[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
    assert_eq!(
        adapter.check_availability(PlatformFeature::DiskStorage),
        Availability::NotImplemented
    );

    #[cfg(any(windows, target_os = "linux", target_os = "macos"))]
    {
        assert_eq!(
            adapter.check_availability(PlatformFeature::VirtualMemoryMap),
            Availability::Supported
        );
        assert_eq!(
            adapter.check_availability(PlatformFeature::MemoryBytes),
            Availability::Supported
        );
    }

    #[cfg(any(windows, target_os = "linux"))]
    assert_eq!(
        adapter.check_availability(PlatformFeature::CacheTopology),
        Availability::Supported
    );

    #[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
    {
        assert_eq!(
            adapter.check_availability(PlatformFeature::VirtualMemoryMap),
            Availability::NotImplemented
        );
        assert_eq!(
            adapter.check_availability(PlatformFeature::MemoryBytes),
            Availability::NotImplemented
        );
    }

    #[cfg(not(any(windows, target_os = "linux")))]
    assert_eq!(
        adapter.check_availability(PlatformFeature::CacheTopology),
        Availability::NotImplemented
    );

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

    #[cfg(any(windows, target_os = "linux"))]
    {
        assert_eq!(
            adapter.check_availability(PlatformFeature::ProcessEnumeration),
            Availability::Supported
        );
        assert_eq!(
            adapter.check_availability(PlatformFeature::ThreadEnumeration),
            Availability::Supported
        );
    }

    #[cfg(any(all(windows, target_arch = "x86_64"), target_os = "linux"))]
    assert_eq!(
        adapter.check_availability(PlatformFeature::ForeignThreadRegisters),
        Availability::Supported
    );
    #[cfg(not(any(all(windows, target_arch = "x86_64"), target_os = "linux")))]
    assert_eq!(
        adapter.check_availability(PlatformFeature::ForeignThreadRegisters),
        Availability::NotImplemented
    );

    assert_eq!(
        adapter.check_availability(PlatformFeature::ForeignProcessMemory),
        Availability::NotImplemented
    );

    #[cfg(target_os = "linux")]
    assert_eq!(
        adapter.check_availability(PlatformFeature::CacheCounters),
        Availability::Supported
    );
    #[cfg(windows)]
    assert_eq!(
        adapter.check_availability(PlatformFeature::CacheCounters),
        Availability::Unavailable
    );
    #[cfg(not(any(windows, target_os = "linux")))]
    assert_eq!(
        adapter.check_availability(PlatformFeature::CacheCounters),
        Availability::NotImplemented
    );
}

#[cfg(any(windows, target_os = "linux"))]
use u_monitor_core::DataSource;
use u_monitor_core::{Availability, PlatformAdapter, PlatformFeature, PlatformResult};
#[cfg(any(windows, target_os = "linux"))]
use u_monitor_platform::NativePlatformAdapter;
use u_monitor_platform::StubPlatformAdapter;

#[test]
fn stub_does_not_invent_cache_topology() {
    let adapter = StubPlatformAdapter::new();
    assert_eq!(
        adapter.check_availability(PlatformFeature::CacheTopology),
        Availability::NotImplemented
    );
    assert_eq!(
        adapter.check_availability(PlatformFeature::CacheCounters),
        Availability::NotImplemented
    );
    assert!(matches!(
        adapter.cache_topology(),
        PlatformResult::NotImplemented(_)
    ));
    assert!(matches!(
        adapter.cache_counters(),
        PlatformResult::NotImplemented(_)
    ));
}

#[test]
#[cfg(any(windows, target_os = "linux"))]
fn native_cache_topology_is_measured() {
    let adapter = NativePlatformAdapter;
    assert_eq!(
        adapter.check_availability(PlatformFeature::CacheTopology),
        Availability::Supported
    );
    let PlatformResult::Success(topology) = adapter.cache_topology() else {
        panic!("expected cache topology");
    };
    assert!(!topology.value.nodes.is_empty());
    assert!(topology.value.nodes.iter().any(|node| node.level >= 1));
    assert_eq!(topology.source, DataSource::Measured);
    for node in &topology.value.nodes {
        if let Some(size) = node.size_bytes {
            assert!(size > 0);
        }
        if let Some(line) = node.line_size_bytes {
            assert!(line > 0);
        }
    }
}

#[test]
#[cfg(windows)]
fn native_windows_cache_counters_are_unavailable_not_simulated() {
    let adapter = NativePlatformAdapter;
    assert_eq!(
        adapter.check_availability(PlatformFeature::CacheCounters),
        Availability::Unavailable
    );
    assert!(matches!(
        adapter.cache_counters(),
        PlatformResult::Unavailable(_)
    ));
}

#[test]
#[cfg(target_os = "linux")]
fn native_linux_cache_counters_are_measured_permissioned_or_unavailable() {
    let adapter = NativePlatformAdapter;
    assert_eq!(
        adapter.check_availability(PlatformFeature::CacheCounters),
        Availability::Supported
    );
    match adapter.cache_counters() {
        PlatformResult::Success(counters) => {
            assert_eq!(counters.source, DataSource::Measured);
            assert!(matches!(
                counters.value.references,
                PlatformResult::Success(_)
                    | PlatformResult::Unavailable(_)
                    | PlatformResult::PermissionRequired(_)
            ));
            assert!(matches!(
                counters.value.misses,
                PlatformResult::Success(_)
                    | PlatformResult::Unavailable(_)
                    | PlatformResult::PermissionRequired(_)
            ));
        }
        PlatformResult::PermissionRequired(_) | PlatformResult::Unavailable(_) => {}
        other => panic!("unexpected cache counters result: {other:?}"),
    }
}

use u_monitor_core::{Availability, DataSource, PlatformAdapter, PlatformFeature, PlatformResult};
use u_monitor_platform::{NativePlatformAdapter, StubPlatformAdapter};

#[test]
fn stub_does_not_invent_cache_topology() {
    let adapter = StubPlatformAdapter::new();
    assert_eq!(
        adapter.check_availability(PlatformFeature::CacheTopology),
        Availability::NotImplemented
    );
    assert!(matches!(
        adapter.cache_topology(),
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

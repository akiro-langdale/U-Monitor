use u_monitor_core::{Availability, PlatformAdapter, PlatformFeature, PlatformResult};
use u_monitor_platform::{NativePlatformAdapter, StubPlatformAdapter};

#[test]
fn stub_does_not_invent_virtual_memory_data() {
    let adapter = StubPlatformAdapter::new();
    assert_eq!(
        adapter.check_availability(PlatformFeature::VirtualMemoryMap),
        Availability::NotImplemented
    );
    assert!(matches!(
        adapter.virtual_memory_regions(),
        PlatformResult::NotImplemented(_)
    ));
    assert!(matches!(
        adapter.read_current_process_bytes(u_monitor_core::MemoryReadRequest {
            address: 0,
            length: 16,
        }),
        PlatformResult::NotImplemented(_)
    ));
}

#[test]
#[cfg(any(windows, target_os = "linux"))]
fn native_map_and_readable_sample_are_measured() {
    let adapter = NativePlatformAdapter;
    assert_eq!(
        adapter.check_availability(PlatformFeature::VirtualMemoryMap),
        Availability::Supported
    );
    assert_eq!(
        adapter.check_availability(PlatformFeature::MemoryBytes),
        Availability::Supported
    );
    let PlatformResult::Success(regions) = adapter.virtual_memory_regions() else {
        panic!("expected current-process region map");
    };
    assert!(!regions.value.is_empty());
    let readable = regions
        .value
        .iter()
        .find(|region| region.readable && region.size_bytes >= 16)
        .expect("readable region");
    let PlatformResult::Success(sample) =
        adapter.read_current_process_bytes(u_monitor_core::MemoryReadRequest {
            address: readable.base_address,
            length: 16,
        })
    else {
        panic!("expected readable sample");
    };
    assert_eq!(sample.value.bytes.len(), 16);
    assert_eq!(sample.source, u_monitor_core::DataSource::Measured);
}

use u_monitor_core::{Availability, PlatformAdapter, PlatformFeature, PlatformResult};
use u_monitor_platform::{NativePlatformAdapter, StubPlatformAdapter};

#[test]
fn stub_does_not_invent_register_snapshots() {
    let adapter = StubPlatformAdapter::new();
    assert_eq!(
        adapter.check_availability(PlatformFeature::CurrentThreadRegisters),
        Availability::NotImplemented
    );
    assert!(matches!(
        adapter.current_thread_registers(),
        PlatformResult::NotImplemented(_)
    ));
}

#[test]
#[cfg(all(windows, target_arch = "x86_64"))]
fn native_windows_x64_current_thread_registers_are_measured() {
    let adapter = NativePlatformAdapter;
    assert_eq!(
        adapter.check_availability(PlatformFeature::CurrentThreadRegisters),
        Availability::Supported
    );
    let PlatformResult::Success(snapshot) = adapter.current_thread_registers() else {
        panic!("expected register snapshot");
    };
    assert_eq!(
        snapshot.value.architecture,
        u_monitor_core::RegisterArchitecture::X86_64
    );
    assert!(!snapshot.value.registers.is_empty());
    assert_eq!(snapshot.source, u_monitor_core::DataSource::Measured);
}

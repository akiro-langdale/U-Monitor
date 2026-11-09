use u_monitor_core::{Availability, PlatformAdapter, PlatformFeature, PlatformResult};
use u_monitor_platform::{NativePlatformAdapter, StubPlatformAdapter};

#[test]
fn stub_does_not_invent_processes() {
    let adapter = StubPlatformAdapter::new();
    assert_eq!(
        adapter.check_availability(PlatformFeature::ProcessEnumeration),
        Availability::NotImplemented
    );
    assert!(matches!(
        adapter.list_processes(),
        PlatformResult::NotImplemented(_)
    ));
}

#[test]
#[cfg(any(windows, target_os = "linux", target_os = "macos"))]
fn native_lists_current_process() {
    let adapter = NativePlatformAdapter;
    assert_eq!(
        adapter.check_availability(PlatformFeature::ProcessEnumeration),
        Availability::Supported
    );
    let PlatformResult::Success(list) = adapter.list_processes() else {
        panic!("expected process list");
    };
    assert!(!list.value.is_empty());
    assert!(list.value.iter().any(|process| process.is_current));
    assert_eq!(list.source, u_monitor_core::DataSource::Measured);
}

#[test]
#[cfg(any(windows, target_os = "linux"))]
fn native_lists_threads_for_current_process() {
    let adapter = NativePlatformAdapter;
    let PlatformResult::Success(list) = adapter.list_processes() else {
        panic!("expected process list");
    };
    let current = list
        .value
        .iter()
        .find(|process| process.is_current)
        .expect("current process");
    let PlatformResult::Success(threads) = adapter.list_threads(current.pid) else {
        panic!("expected thread list");
    };
    assert!(!threads.value.is_empty());
    assert!(
        threads
            .value
            .iter()
            .all(|thread| thread.process_pid == current.pid)
    );
}

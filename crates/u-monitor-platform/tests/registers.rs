#[cfg(any(all(windows, target_arch = "x86_64"), target_os = "linux"))]
use std::{sync::mpsc, thread, time::Duration};
use u_monitor_core::{Availability, PlatformAdapter, PlatformFeature, PlatformResult};
#[cfg(any(all(windows, target_arch = "x86_64"), target_os = "linux"))]
use u_monitor_platform::NativePlatformAdapter;
use u_monitor_platform::StubPlatformAdapter;

#[test]
fn stub_does_not_invent_register_snapshots() {
    let adapter = StubPlatformAdapter::new();
    assert_eq!(
        adapter.check_availability(PlatformFeature::CurrentThreadRegisters),
        Availability::NotImplemented
    );
    assert_eq!(
        adapter.check_availability(PlatformFeature::ForeignThreadRegisters),
        Availability::NotImplemented
    );
    assert!(matches!(
        adapter.current_thread_registers(),
        PlatformResult::NotImplemented(_)
    ));
    assert!(matches!(
        adapter.thread_registers(1, 1),
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

#[test]
#[cfg(any(all(windows, target_arch = "x86_64"), target_os = "linux"))]
fn native_other_thread_registers_are_measured_or_permissioned() {
    let adapter = NativePlatformAdapter;
    assert_eq!(
        adapter.check_availability(PlatformFeature::ForeignThreadRegisters),
        Availability::Supported
    );

    let (tid_tx, tid_rx) = mpsc::channel();
    let (done_tx, done_rx) = mpsc::channel();
    let handle = thread::spawn(move || {
        tid_tx
            .send(current_os_tid())
            .expect("send helper thread id");
        // Stay alive while the parent captures CONTEXT / ptrace.
        let _ = done_rx.recv_timeout(Duration::from_secs(5));
    });

    let tid = tid_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("helper thread id");
    let pid = u64::from(std::process::id());
    let result = adapter.thread_registers(pid, tid);
    let _ = done_tx.send(());
    handle.join().expect("helper thread");

    match result {
        PlatformResult::Success(snapshot) => {
            assert!(!snapshot.value.registers.is_empty());
            assert_eq!(snapshot.source, u_monitor_core::DataSource::Measured);
            match &snapshot.value.thread_id {
                PlatformResult::Success(measured_tid) => assert_eq!(measured_tid.value, tid),
                other => panic!("expected measured thread id, got {other:?}"),
            }
        }
        PlatformResult::PermissionRequired(_) => {}
        other => panic!("unexpected other-thread register result: {other:?}"),
    }
}

#[cfg(all(windows, target_arch = "x86_64"))]
fn current_os_tid() -> u64 {
    u64::from(unsafe { windows::Win32::System::Threading::GetCurrentThreadId() })
}

#[cfg(target_os = "linux")]
fn current_os_tid() -> u64 {
    u64::try_from(unsafe { libc::syscall(libc::SYS_gettid) }).expect("tid fits u64")
}

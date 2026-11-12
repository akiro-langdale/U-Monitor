use u_monitor_core::{Availability, PlatformAdapter, PlatformFeature, PlatformResult};
use u_monitor_platform::{NativePlatformAdapter, StubPlatformAdapter};

#[cfg(windows)]
#[test]
fn reads_known_bytes_from_child_process() {
    use std::{
        io::{BufRead, BufReader, Write},
        process::{Command, Stdio},
    };
    use u_monitor_core::MemoryReadRequest;
    use u_monitor_platform::ProcessMemory;

    static PAYLOAD: &[u8] = b"u-monitor-child-memory";
    const CHILD: &str = "U_MONITOR_MEMORY_TEST_CHILD";
    if std::env::var_os(CHILD).is_some() {
        println!("PAYLOAD_ADDRESS={}", PAYLOAD.as_ptr() as usize);
        std::io::stdout().flush().unwrap();
        let mut input = String::new();
        std::io::stdin().read_line(&mut input).unwrap();
        return;
    }

    let mut child = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "reads_known_bytes_from_child_process",
            "--nocapture",
        ])
        .env(CHILD, "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("start memory test child");
    let mut stdout = BufReader::new(child.stdout.take().unwrap());
    let address = (&mut stdout)
        .lines()
        .map_while(Result::ok)
        .find_map(|line| {
            line.split_once("PAYLOAD_ADDRESS=")?
                .1
                .trim()
                .parse::<u64>()
                .ok()
        });

    let sample = address.and_then(|address| {
        let PlatformResult::Success(session) = ProcessMemory::open(child.id().into()) else {
            return None;
        };
        let PlatformResult::Success(regions) = session.value.regions() else {
            return None;
        };
        let readable = regions.value.iter().any(|region| {
            region.readable
                && address >= region.base_address
                && address + PAYLOAD.len() as u64 <= region.base_address + region.size_bytes
        });
        let request = MemoryReadRequest {
            address,
            length: PAYLOAD.len(),
        };
        let PlatformResult::Success(sample) = session.value.read(request) else {
            return None;
        };
        Some((readable, sample.value.bytes, session.value, request))
    });
    // Release the child before asserting so failures leave no waiting process.
    drop(child.stdin.take());
    std::io::copy(&mut stdout, &mut std::io::sink()).unwrap();
    assert!(child.wait().unwrap().success());
    let (readable, bytes, previous_target, request) = sample.expect("sample from live child");
    assert!(readable);
    assert_eq!(bytes, PAYLOAD);

    let PlatformResult::Success(next_target) = ProcessMemory::open(std::process::id().into())
    else {
        panic!("expected to open the new target");
    };
    let local = Box::new(*b"current process");
    let PlatformResult::Success(sample) = next_target.value.read(MemoryReadRequest {
        address: local.as_ptr() as usize as u64,
        length: local.len(),
    }) else {
        panic!("expected sample from the new target");
    };
    assert_eq!(sample.value.bytes, local.as_slice());
    assert!(matches!(
        previous_target.regions(),
        PlatformResult::Unavailable(_)
    ));
    assert!(matches!(
        previous_target.read(request),
        PlatformResult::Unavailable(_)
    ));
}

#[cfg(windows)]
#[test]
fn protected_system_process_requires_permission() {
    let PlatformResult::Success(processes) = NativePlatformAdapter.list_processes() else {
        panic!("expected Windows process list");
    };
    let system = processes
        .value
        .iter()
        .find(|process| process.name.eq_ignore_ascii_case("System"))
        .expect("expected the Windows System process");
    assert!(matches!(
        u_monitor_platform::ProcessMemory::open(system.pid),
        PlatformResult::PermissionRequired(_)
    ));
}

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
#[cfg(any(windows, target_os = "linux", target_os = "macos"))]
fn native_map_and_readable_sample_are_measured() {
    let payload = Box::new([0x55_u8, 0x00, 0xFF, 0x4C]);
    let address = payload.as_ptr() as usize as u64;
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
    assert!(regions.value.iter().any(|region| {
        region.readable
            && address >= region.base_address
            && address + payload.len() as u64
                <= region.base_address.saturating_add(region.size_bytes)
    }));
    let PlatformResult::Success(sample) =
        adapter.read_current_process_bytes(u_monitor_core::MemoryReadRequest {
            address,
            length: payload.len(),
        })
    else {
        panic!("expected readable sample");
    };
    assert_eq!(sample.value.bytes, payload.as_slice());
    assert_eq!(sample.source, u_monitor_core::DataSource::Measured);
}

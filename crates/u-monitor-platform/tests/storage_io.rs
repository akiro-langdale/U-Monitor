use std::{fs, io::Write, time::Duration};
use u_monitor_core::{
    Availability, FileReadRequest, PlatformAdapter, PlatformFeature, PlatformResult,
};
use u_monitor_platform::{
    MAX_FILE_SAMPLE_BYTES, MINIMUM_DISK_IO_INTERVAL, NativePlatformAdapter, NativeTelemetryAdapter,
    StubPlatformAdapter,
};

#[test]
fn stub_does_not_invent_disk_io_or_file_bytes() {
    let mut stub = StubPlatformAdapter::new();
    assert_eq!(
        stub.check_availability(PlatformFeature::DiskStorage),
        Availability::NotImplemented
    );
    assert!(matches!(
        stub.disk_io_rates(),
        PlatformResult::NotImplemented(_)
    ));
    assert!(matches!(
        stub.read_file_bytes(FileReadRequest {
            path: "missing.bin".into(),
            offset: 0,
            length: 16,
        }),
        PlatformResult::NotImplemented(_)
    ));
}

#[test]
fn native_file_hex_reads_regular_file_and_rejects_missing() {
    let adapter = NativePlatformAdapter;
    let dir = std::env::temp_dir().join("u-monitor-file-hex-test");
    let _ = fs::create_dir_all(&dir);
    let path = dir.join("sample.bin");
    {
        let mut file = fs::File::create(&path).expect("create temp file");
        file.write_all(b"Hello, U-Monitor HEX\n")
            .expect("write temp file");
    }

    let PlatformResult::Success(sample) = adapter.read_file_bytes(FileReadRequest {
        path: path.clone(),
        offset: 0,
        length: MAX_FILE_SAMPLE_BYTES,
    }) else {
        panic!("expected measured file bytes");
    };
    assert_eq!(sample.source, u_monitor_core::DataSource::Measured);
    assert!(sample.value.bytes.starts_with(b"Hello"));

    assert!(matches!(
        adapter.read_file_bytes(FileReadRequest {
            path: dir.join("missing.bin"),
            offset: 0,
            length: 8,
        }),
        PlatformResult::Unavailable(_)
    ));
}

#[test]
#[cfg(any(windows, target_os = "linux"))]
fn native_disk_io_warmup_then_rate_or_permission() {
    let mut adapter = NativeTelemetryAdapter::default();
    assert_eq!(
        adapter.check_availability(PlatformFeature::DiskStorage),
        Availability::Supported
    );
    match adapter.disk_io_rates() {
        PlatformResult::Unavailable(_) => {}
        PlatformResult::PermissionRequired(_) => return,
        other => panic!("expected warmup unavailable, got {other:?}"),
    }
    std::thread::sleep(MINIMUM_DISK_IO_INTERVAL + Duration::from_millis(50));
    match adapter.disk_io_rates() {
        PlatformResult::Success(rates) => {
            assert_eq!(rates.source, u_monitor_core::DataSource::Calculated);
            assert!(matches!(
                rates.value.read_bytes_per_sec,
                PlatformResult::Success(_)
            ));
            assert!(matches!(
                rates.value.write_bytes_per_sec,
                PlatformResult::Success(_)
            ));
        }
        PlatformResult::PermissionRequired(_) | PlatformResult::Unavailable(_) => {}
        other => panic!("unexpected disk io result: {other:?}"),
    }
}

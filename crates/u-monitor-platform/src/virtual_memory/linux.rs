//! Linux current-process map via `/proc/self/maps` and `/proc/self/mem`.

use super::{MAX_MEMORY_SAMPLE_BYTES, maps_parse::parse_maps_line};
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
    time::SystemTime,
};
use u_monitor_core::{
    DataSource, DiagnosticError, Measurement, MemoryBytes, MemoryReadRequest, PlatformResult,
    VirtualMemoryRegion,
};

fn measured<T>(value: T) -> PlatformResult<T> {
    PlatformResult::Success(Measurement {
        value,
        source: DataSource::Measured,
        timestamp: SystemTime::now().into(),
    })
}

pub(super) fn enumerate_regions() -> PlatformResult<Vec<VirtualMemoryRegion>> {
    let text = match std::fs::read_to_string("/proc/self/maps") {
        Ok(text) => text,
        Err(error) => {
            return PlatformResult::Error(DiagnosticError::PlatformError(format!(
                "failed to read /proc/self/maps: {error}"
            )));
        }
    };
    let mut regions = Vec::new();
    for line in text.lines() {
        if let Some(region) = parse_maps_line(line) {
            regions.push(region);
        }
    }
    if regions.is_empty() {
        return PlatformResult::Unavailable("No virtual memory regions were reported".into());
    }
    measured(regions)
}

pub(super) fn read_bytes(request: MemoryReadRequest) -> PlatformResult<MemoryBytes> {
    if request.length == 0 {
        return PlatformResult::Error(DiagnosticError::ValidationError(
            "Memory sample length must be greater than zero".into(),
        ));
    }
    let length = request.length.min(MAX_MEMORY_SAMPLE_BYTES);
    let end = match request.address.checked_add(length as u64) {
        Some(value) => value,
        None => {
            return PlatformResult::Error(DiagnosticError::ValidationError(
                "Memory sample range overflows the address space".into(),
            ));
        }
    };

    let regions = match enumerate_regions() {
        PlatformResult::Success(measurement) => measurement.value,
        PlatformResult::PermissionRequired(reason) => {
            return PlatformResult::PermissionRequired(reason);
        }
        PlatformResult::Unavailable(reason) => return PlatformResult::Unavailable(reason),
        PlatformResult::NotImplemented(reason) => {
            return PlatformResult::NotImplemented(reason);
        }
        PlatformResult::Error(error) => return PlatformResult::Error(error),
    };

    if !regions.iter().any(|region| {
        region.readable
            && request.address >= region.base_address
            && end <= region.base_address.saturating_add(region.size_bytes)
    }) {
        return PlatformResult::Unavailable(
            "Requested address is not in a readable mapped region".into(),
        );
    }

    let mut file = match File::open("/proc/self/mem") {
        Ok(file) => file,
        Err(error) => {
            return PlatformResult::Error(DiagnosticError::PlatformError(format!(
                "failed to open /proc/self/mem: {error}"
            )));
        }
    };
    if let Err(error) = file.seek(SeekFrom::Start(request.address)) {
        return PlatformResult::Unavailable(format!(
            "cannot seek /proc/self/mem to 0x{:X}: {error}",
            request.address
        ));
    }
    let mut bytes = vec![0u8; length];
    match file.read_exact(&mut bytes) {
        Ok(()) => measured(MemoryBytes {
            address: request.address,
            bytes,
        }),
        Err(error) => PlatformResult::Unavailable(format!(
            "failed to read {length} bytes from /proc/self/mem: {error}"
        )),
    }
}

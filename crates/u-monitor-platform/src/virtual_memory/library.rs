//! Adapts U-Library measurements to the monitor platform contract.

use std::time::SystemTime;
use u_library_memory::process_memory::{self, ProcessMemoryError};
use u_monitor_core::{
    DataSource, DiagnosticError, Measurement, MemoryBytes, MemoryProtection, MemoryReadRequest,
    MemoryRegionKind, MemoryState, PlatformResult, VirtualMemoryRegion,
};

pub struct ProcessMemory(process_memory::ProcessMemory);

impl ProcessMemory {
    pub fn open(pid: u64) -> PlatformResult<Self> {
        measured(process_memory::ProcessMemory::open(pid).map(Self))
    }

    pub fn regions(&self) -> PlatformResult<Vec<VirtualMemoryRegion>> {
        adapt_regions(self.0.regions())
    }

    pub fn read(&self, request: MemoryReadRequest) -> PlatformResult<MemoryBytes> {
        adapt_sample(
            request.address,
            self.0.read(request.address, request.length),
        )
    }
}

pub(super) fn enumerate_regions() -> PlatformResult<Vec<VirtualMemoryRegion>> {
    adapt_regions(process_memory::current_process_regions())
}

pub(super) fn read_bytes(request: MemoryReadRequest) -> PlatformResult<MemoryBytes> {
    adapt_sample(
        request.address,
        process_memory::read_current_process(request.address, request.length),
    )
}

fn adapt_sample(
    address: u64,
    result: Result<Vec<u8>, ProcessMemoryError>,
) -> PlatformResult<MemoryBytes> {
    measured(result.map(|bytes| MemoryBytes { address, bytes }))
}

fn adapt_regions(
    result: Result<Vec<process_memory::VirtualMemoryRegion>, ProcessMemoryError>,
) -> PlatformResult<Vec<VirtualMemoryRegion>> {
    use process_memory::{MemoryRegionKind as Kind, MemoryState as State};

    measured(result.map(|regions| {
        regions
            .into_iter()
            .map(|region| VirtualMemoryRegion {
                base_address: region.base_address,
                size_bytes: region.size_bytes,
                state: match region.state {
                    State::Free => MemoryState::Free,
                    State::Reserved => MemoryState::Reserved,
                    State::Committed => MemoryState::Committed,
                    State::Unknown => MemoryState::Unknown,
                },
                protection: MemoryProtection {
                    read: region.protection.read,
                    write: region.protection.write,
                    execute: region.protection.execute,
                    guard: region.protection.guard,
                },
                kind: match region.kind {
                    Kind::Private => MemoryRegionKind::Private,
                    Kind::Image => MemoryRegionKind::Image,
                    Kind::Mapped => MemoryRegionKind::Mapped,
                    Kind::Free => MemoryRegionKind::Free,
                    Kind::Unknown => MemoryRegionKind::Unknown,
                },
                readable: region.readable,
            })
            .collect()
    }))
}

fn measured<T>(result: Result<T, ProcessMemoryError>) -> PlatformResult<T> {
    match result {
        Ok(value) => PlatformResult::Success(Measurement {
            value,
            source: DataSource::Measured,
            timestamp: SystemTime::now().into(),
        }),
        Err(ProcessMemoryError::InvalidRange(message)) => {
            PlatformResult::Error(DiagnosticError::ValidationError(message))
        }
        Err(ProcessMemoryError::EnumerationLimit(message)) => {
            PlatformResult::Error(DiagnosticError::PlatformError(message))
        }
        Err(ProcessMemoryError::PermissionRequired(message)) => {
            PlatformResult::PermissionRequired(message)
        }
        Err(ProcessMemoryError::Unavailable(message)) => PlatformResult::Unavailable(message),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_process_map_contains_readable_committed_region() {
        let PlatformResult::Success(regions) = enumerate_regions() else {
            panic!("expected regions");
        };
        assert!(!regions.value.is_empty());
        assert!(regions.value.iter().any(|region| region.readable));
        let sample_region = regions
            .value
            .iter()
            .find(|region| region.readable && region.size_bytes >= 16)
            .expect("readable region");
        let PlatformResult::Success(sample) = read_bytes(MemoryReadRequest {
            address: sample_region.base_address,
            length: 16,
        }) else {
            panic!("expected readable sample");
        };
        assert_eq!(sample.value.address, sample_region.base_address);
        assert_eq!(sample.value.bytes.len(), 16);
    }

    #[test]
    fn zero_length_and_unreadable_requests_do_not_fabricate_bytes() {
        assert!(matches!(
            read_bytes(MemoryReadRequest {
                address: 0,
                length: 0,
            }),
            PlatformResult::Error(_)
        ));
        // NULL page is not a valid readable committed mapping for this inspector.
        assert!(matches!(
            read_bytes(MemoryReadRequest {
                address: 0,
                length: 16,
            }),
            PlatformResult::Unavailable(_) | PlatformResult::Error(_)
        ));
    }
}

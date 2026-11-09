//! Windows VirtualQuery / safe current-process reads.

use super::MAX_MEMORY_SAMPLE_BYTES;
use std::{
    mem::{MaybeUninit, size_of},
    ptr,
    time::SystemTime,
};
use u_monitor_core::{
    DataSource, DiagnosticError, Measurement, MemoryBytes, MemoryProtection, MemoryReadRequest,
    MemoryRegionKind, MemoryState, PlatformResult, VirtualMemoryRegion,
};
use windows::Win32::System::Memory::{
    MEM_COMMIT, MEM_FREE, MEM_IMAGE, MEM_MAPPED, MEM_PRIVATE, MEM_RESERVE,
    MEMORY_BASIC_INFORMATION, PAGE_EXECUTE, PAGE_EXECUTE_READ, PAGE_EXECUTE_READWRITE,
    PAGE_EXECUTE_WRITECOPY, PAGE_GUARD, PAGE_NOACCESS, PAGE_PROTECTION_FLAGS, PAGE_READONLY,
    PAGE_READWRITE, PAGE_WRITECOPY, VirtualQuery,
};

fn measured<T>(value: T) -> PlatformResult<T> {
    PlatformResult::Success(Measurement {
        value,
        source: DataSource::Measured,
        timestamp: SystemTime::now().into(),
    })
}

fn protection_from(flags: PAGE_PROTECTION_FLAGS) -> MemoryProtection {
    let bits = flags.0;
    let guard = bits & PAGE_GUARD.0 != 0;
    let access = bits & !PAGE_GUARD.0;
    let (read, write, execute) = if access == PAGE_NOACCESS.0 {
        (false, false, false)
    } else if access == PAGE_READONLY.0 {
        (true, false, false)
    } else if access == PAGE_READWRITE.0 || access == PAGE_WRITECOPY.0 {
        (true, true, false)
    } else if access == PAGE_EXECUTE.0 {
        (false, false, true)
    } else if access == PAGE_EXECUTE_READ.0 {
        (true, false, true)
    } else if access == PAGE_EXECUTE_READWRITE.0 || access == PAGE_EXECUTE_WRITECOPY.0 {
        (true, true, true)
    } else {
        // Unknown combinations stay conservative: no implied readability.
        (false, false, false)
    };
    MemoryProtection {
        read,
        write,
        execute,
        guard,
    }
}

fn state_from(state: windows::Win32::System::Memory::VIRTUAL_ALLOCATION_TYPE) -> MemoryState {
    if state == MEM_COMMIT {
        MemoryState::Committed
    } else if state == MEM_RESERVE {
        MemoryState::Reserved
    } else if state == MEM_FREE {
        MemoryState::Free
    } else {
        MemoryState::Unknown
    }
}

fn kind_from(
    region_type: windows::Win32::System::Memory::PAGE_TYPE,
    state: MemoryState,
) -> MemoryRegionKind {
    if state == MemoryState::Free {
        return MemoryRegionKind::Free;
    }
    if region_type == MEM_IMAGE {
        MemoryRegionKind::Image
    } else if region_type == MEM_MAPPED {
        MemoryRegionKind::Mapped
    } else if region_type == MEM_PRIVATE {
        MemoryRegionKind::Private
    } else {
        MemoryRegionKind::Unknown
    }
}

fn region_from(info: &MEMORY_BASIC_INFORMATION) -> VirtualMemoryRegion {
    let state = state_from(info.State);
    let protection = if state == MemoryState::Committed {
        protection_from(info.Protect)
    } else {
        MemoryProtection::none()
    };
    let readable = state == MemoryState::Committed && protection.read && !protection.guard;
    VirtualMemoryRegion {
        base_address: info.BaseAddress as usize as u64,
        size_bytes: info.RegionSize as u64,
        state,
        protection,
        kind: kind_from(info.Type, state),
        readable,
    }
}

pub(super) fn enumerate_regions() -> PlatformResult<Vec<VirtualMemoryRegion>> {
    let mut regions = Vec::new();
    let mut address = 0usize;
    loop {
        let mut info = MaybeUninit::<MEMORY_BASIC_INFORMATION>::uninit();
        // SAFETY: VirtualQuery writes a complete MEMORY_BASIC_INFORMATION on success.
        let written = unsafe {
            VirtualQuery(
                Some(address as *const _),
                info.as_mut_ptr(),
                size_of::<MEMORY_BASIC_INFORMATION>(),
            )
        };
        if written == 0 {
            break;
        }
        // SAFETY: VirtualQuery initialized the buffer when it returned non-zero.
        let info = unsafe { info.assume_init() };
        let region = region_from(&info);
        let next = match (region.base_address as usize).checked_add(region.size_bytes as usize) {
            Some(value) if value > address => value,
            _ => break,
        };
        regions.push(region);
        address = next;
        if regions.len() > 100_000 {
            return PlatformResult::Error(DiagnosticError::PlatformError(
                "Virtual memory enumeration exceeded the safety limit".into(),
            ));
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

    let mut address = request.address as usize;
    let mut remaining = length;
    let mut bytes = Vec::with_capacity(length);

    while remaining > 0 {
        let mut info = MaybeUninit::<MEMORY_BASIC_INFORMATION>::uninit();
        // SAFETY: VirtualQuery writes a complete MEMORY_BASIC_INFORMATION on success.
        let written = unsafe {
            VirtualQuery(
                Some(address as *const _),
                info.as_mut_ptr(),
                size_of::<MEMORY_BASIC_INFORMATION>(),
            )
        };
        if written == 0 {
            return PlatformResult::Unavailable(
                "VirtualQuery could not resolve the requested address".into(),
            );
        }
        // SAFETY: buffer was initialized by a successful VirtualQuery call.
        let info = unsafe { info.assume_init() };
        let region = region_from(&info);
        if !region.readable {
            return PlatformResult::Unavailable(
                "Requested address is not in a readable committed region".into(),
            );
        }
        let region_end = region.base_address.saturating_add(region.size_bytes);
        if (address as u64) < region.base_address || (address as u64) >= region_end {
            return PlatformResult::Unavailable(
                "Requested address is outside the resolved virtual region".into(),
            );
        }
        if end > region_end {
            return PlatformResult::Unavailable(
                "Requested sample crosses into a non-readable or different region".into(),
            );
        }
        let chunk = remaining.min((region_end - address as u64) as usize);
        let mut buffer = vec![0u8; chunk];
        // SAFETY: region is committed + readable + non-guard; copy stays within region_end.
        unsafe {
            ptr::copy_nonoverlapping(address as *const u8, buffer.as_mut_ptr(), chunk);
        }
        bytes.extend_from_slice(&buffer);
        address += chunk;
        remaining -= chunk;
    }

    measured(MemoryBytes {
        address: request.address,
        bytes,
    })
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

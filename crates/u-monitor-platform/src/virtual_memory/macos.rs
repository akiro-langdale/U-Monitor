//! macOS current-process map via `mach_vm_region` and direct VA sampling.

use super::MAX_MEMORY_SAMPLE_BYTES;
use std::{mem, ptr, time::SystemTime};
use u_monitor_core::{
    DataSource, DiagnosticError, Measurement, MemoryBytes, MemoryProtection, MemoryReadRequest,
    MemoryRegionKind, MemoryState, PlatformResult, VirtualMemoryRegion,
};

fn measured<T>(value: T) -> PlatformResult<T> {
    PlatformResult::Success(Measurement {
        value,
        source: DataSource::Measured,
        timestamp: SystemTime::now().into(),
    })
}

pub(super) fn enumerate_regions() -> PlatformResult<Vec<VirtualMemoryRegion>> {
    let task = unsafe { mach2::traps::mach_task_self() };
    let mut address: mach2::vm_types::mach_vm_address_t = 1;
    let mut regions = Vec::new();

    loop {
        let mut size: mach2::vm_types::mach_vm_size_t = 0;
        let mut info: mach2::vm_region::vm_region_basic_info_64 = unsafe { mem::zeroed() };
        let mut count = mach2::vm_region::vm_region_basic_info_64::count();
        let mut object_name: mach2::port::mach_port_t = 0;
        let kr = unsafe {
            mach2::vm::mach_vm_region(
                task,
                &mut address,
                &mut size,
                mach2::vm_region::VM_REGION_BASIC_INFO_64,
                (&mut info as *mut _ as *mut i32),
                &mut count,
                &mut object_name,
            )
        };
        if kr != mach2::kern_return::KERN_SUCCESS {
            break;
        }
        if size == 0 {
            break;
        }

        let read = (info.protection & mach2::vm_prot::VM_PROT_READ) != 0;
        let write = (info.protection & mach2::vm_prot::VM_PROT_WRITE) != 0;
        let execute = (info.protection & mach2::vm_prot::VM_PROT_EXECUTE) != 0;
        regions.push(VirtualMemoryRegion {
            base_address: address,
            size_bytes: size,
            state: MemoryState::Committed,
            protection: MemoryProtection {
                read,
                write,
                execute,
                guard: false,
            },
            kind: if info.shared != 0 {
                MemoryRegionKind::Mapped
            } else {
                MemoryRegionKind::Private
            },
            readable: read,
        });

        let Some(next) = address.checked_add(size) else {
            break;
        };
        address = next;
        if address == 0 {
            break;
        }
    }

    if regions.is_empty() {
        return PlatformResult::Unavailable(
            "mach_vm_region returned no regions for the current task".into(),
        );
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

    let mut bytes = vec![0_u8; length];
    // SAFETY: address was validated against a readable mapped region of this process.
    unsafe {
        ptr::copy_nonoverlapping(request.address as *const u8, bytes.as_mut_ptr(), length);
    }
    measured(MemoryBytes {
        address: request.address,
        bytes,
    })
}

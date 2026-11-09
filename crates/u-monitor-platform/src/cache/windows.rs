//! Windows cache topology via GetLogicalProcessorInformationEx(RelationCache).

use std::{mem::size_of, time::SystemTime};
use u_monitor_core::{
    CacheKind, CacheNode, CacheTopology, DataSource, DiagnosticError, Measurement, PlatformResult,
};
use windows::Win32::Foundation::ERROR_INSUFFICIENT_BUFFER;
use windows::Win32::System::SystemInformation::{
    CacheData, CacheInstruction, CacheTrace, CacheUnified, GetLogicalProcessorInformationEx,
    RelationCache, SYSTEM_LOGICAL_PROCESSOR_INFORMATION_EX,
};

fn measured<T>(value: T) -> PlatformResult<T> {
    PlatformResult::Success(Measurement {
        value,
        source: DataSource::Measured,
        timestamp: SystemTime::now().into(),
    })
}

fn kind_from(value: windows::Win32::System::SystemInformation::PROCESSOR_CACHE_TYPE) -> CacheKind {
    if value == CacheData {
        CacheKind::Data
    } else if value == CacheInstruction {
        CacheKind::Instruction
    } else if value == CacheUnified {
        CacheKind::Unified
    } else if value == CacheTrace {
        CacheKind::Trace
    } else {
        CacheKind::Unknown
    }
}

fn processors_from_mask(mask: usize) -> Vec<usize> {
    (0..usize::BITS as usize)
        .filter(|bit| mask & (1usize << bit) != 0)
        .collect()
}

pub(super) fn discover() -> PlatformResult<CacheTopology> {
    let mut length = 0u32;
    // First call asks for the required buffer size.
    match unsafe { GetLogicalProcessorInformationEx(RelationCache, None, &mut length) } {
        Ok(()) => {
            return PlatformResult::Unavailable(
                "Cache topology buffer size was not reported".into(),
            );
        }
        Err(error) if error.code() == ERROR_INSUFFICIENT_BUFFER.into() && length > 0 => {}
        Err(error) => {
            return PlatformResult::Error(DiagnosticError::PlatformError(format!(
                "GetLogicalProcessorInformationEx size probe failed: {error}"
            )));
        }
    }

    let mut buffer = vec![0u8; length as usize];
    if let Err(error) = unsafe {
        GetLogicalProcessorInformationEx(
            RelationCache,
            Some(buffer.as_mut_ptr().cast()),
            &mut length,
        )
    } {
        return PlatformResult::Error(DiagnosticError::PlatformError(format!(
            "GetLogicalProcessorInformationEx failed: {error}"
        )));
    }

    // Record Size is the OS byte length of that entry. Do not compare it to
    // sizeof(SYSTEM_LOGICAL_PROCESSOR_INFORMATION_EX): the Rust layout includes
    // the largest union arm and is often larger than a RelationCache record.
    const HEADER_BYTES: usize = size_of::<u32>() * 2;
    let mut nodes = Vec::new();
    let mut offset = 0usize;
    let usable = length as usize;
    while offset < usable {
        if offset + HEADER_BYTES > buffer.len() || offset + HEADER_BYTES > usable {
            return PlatformResult::Error(DiagnosticError::PlatformError(
                "Truncated cache topology header".into(),
            ));
        }
        // SAFETY: offset is within the OS-reported buffer; header fields sit at record start.
        let info_ptr = unsafe {
            buffer.as_ptr().add(offset) as *const SYSTEM_LOGICAL_PROCESSOR_INFORMATION_EX
        };
        let size = unsafe { (*info_ptr).Size as usize };
        if size < HEADER_BYTES || offset + size > buffer.len() || offset + size > usable {
            return PlatformResult::Error(DiagnosticError::PlatformError(
                "Malformed cache topology record size".into(),
            ));
        }
        // SAFETY: Relationship is within the validated record header.
        let relationship = unsafe { (*info_ptr).Relationship };
        if relationship == RelationCache {
            // SAFETY: RelationCache selects the Cache arm; fields used below fit
            // in a normal single-group cache record reported by Windows.
            let cache = unsafe { (*info_ptr).Anonymous.Cache };
            let fully_associative = cache.Associativity == 0xff;
            let associativity = if fully_associative || cache.Associativity == 0 {
                None
            } else {
                Some(u32::from(cache.Associativity))
            };
            let size_bytes = if cache.CacheSize == 0 {
                None
            } else {
                Some(u64::from(cache.CacheSize))
            };
            let line_size_bytes = if cache.LineSize == 0 {
                None
            } else {
                Some(u32::from(cache.LineSize))
            };
            // SAFETY: GroupMask is the first affinity when GroupCount >= 1.
            let mask = unsafe { cache.Anonymous.GroupMask.Mask };
            nodes.push(CacheNode {
                level: u32::from(cache.Level),
                kind: kind_from(cache.Type),
                size_bytes,
                line_size_bytes,
                associativity,
                fully_associative,
                shared_logical_processors: processors_from_mask(mask),
            });
        }
        offset += size;
    }

    if nodes.is_empty() {
        return PlatformResult::Unavailable("No cache topology entries were reported".into());
    }
    nodes.sort_by_key(|node| (node.level, node.kind as u8));
    measured(CacheTopology { nodes })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discover_reports_at_least_one_cache_level_on_windows() {
        match discover() {
            PlatformResult::Success(topology) => {
                assert!(!topology.value.nodes.is_empty());
                assert!(topology.value.nodes.iter().any(|node| node.level >= 1));
            }
            other => panic!("expected cache topology, got {other:?}"),
        }
    }
}

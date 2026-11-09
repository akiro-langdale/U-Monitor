//! Virtual address space contracts. These describe OS-visible regions, not
//! physical RAM cells. Foreign-process inspection is intentionally separate.

use std::fmt;

/// Commitment state of a virtual address range.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryState {
    Free,
    Reserved,
    Committed,
    Unknown,
}

impl fmt::Display for MemoryState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Free => "FREE",
            Self::Reserved => "RESERVED",
            Self::Committed => "COMMITTED",
            Self::Unknown => "UNKNOWN",
        })
    }
}

/// High-level access rights derived from OS protection flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemoryProtection {
    pub read: bool,
    pub write: bool,
    pub execute: bool,
    pub guard: bool,
}

impl MemoryProtection {
    pub fn none() -> Self {
        Self {
            read: false,
            write: false,
            execute: false,
            guard: false,
        }
    }

    pub fn label(self) -> String {
        let mut label = String::new();
        label.push(if self.read { 'R' } else { '-' });
        label.push(if self.write { 'W' } else { '-' });
        label.push(if self.execute { 'X' } else { '-' });
        if self.guard {
            label.push('G');
        }
        label
    }
}

/// Backing classification reported by the OS for a region.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryRegionKind {
    Private,
    Image,
    Mapped,
    Free,
    Unknown,
}

impl fmt::Display for MemoryRegionKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Private => "PRIVATE",
            Self::Image => "IMAGE",
            Self::Mapped => "MAPPED",
            Self::Free => "FREE",
            Self::Unknown => "UNKNOWN",
        })
    }
}

/// One contiguous virtual region in a process address space.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VirtualMemoryRegion {
    pub base_address: u64,
    pub size_bytes: u64,
    pub state: MemoryState,
    pub protection: MemoryProtection,
    pub kind: MemoryRegionKind,
    /// Whether the inspector may attempt a read-only byte sample.
    pub readable: bool,
}

/// A bounded read-only sample from an allowed virtual range.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryBytes {
    pub address: u64,
    pub bytes: Vec<u8>,
}

/// Request for a read-only sample. Length is capped by the platform adapter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemoryReadRequest {
    pub address: u64,
    pub length: usize,
}

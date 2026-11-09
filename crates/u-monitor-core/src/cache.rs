//! CPU cache topology contracts. Describes hierarchy metadata, never cache-line bytes.

use std::fmt;

/// Which contents a cache level may hold, as reported by the OS/firmware.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheKind {
    Data,
    Instruction,
    Unified,
    Trace,
    Unknown,
}

impl fmt::Display for CacheKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Data => "DATA",
            Self::Instruction => "INSTRUCTION",
            Self::Unified => "UNIFIED",
            Self::Trace => "TRACE",
            Self::Unknown => "UNKNOWN",
        })
    }
}

/// One OS-reported cache node in the hierarchy.
/// Missing optional fields stay absent; zeros from backends are treated as missing
/// unless the OS explicitly documents a valid zero (associativity 0 sometimes means
/// fully associative — captured separately when known).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CacheNode {
    pub level: u32,
    pub kind: CacheKind,
    pub size_bytes: Option<u64>,
    pub line_size_bytes: Option<u32>,
    pub associativity: Option<u32>,
    /// True when the OS reports associativity 0 as "fully associative".
    pub fully_associative: bool,
    /// Logical processor indices sharing this cache, when the OS exposes them.
    pub shared_logical_processors: Vec<usize>,
}

/// Complete topology snapshot for the host as visible to this process.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CacheTopology {
    pub nodes: Vec<CacheNode>,
}

//! CPU register snapshot contracts. Values are ISA/OS-specific and never invented.

use crate::PlatformResult;
use std::fmt;

/// Instruction-set family of a register snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegisterArchitecture {
    X86_64,
    AArch64,
    Unknown(String),
}

impl fmt::Display for RegisterArchitecture {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::X86_64 => f.write_str("x86-64"),
            Self::AArch64 => f.write_str("AArch64"),
            Self::Unknown(name) => write!(f, "unknown:{name}"),
        }
    }
}

/// One named general-purpose or status register.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisterValue {
    pub name: String,
    pub bits: u64,
    pub width_bits: u8,
}

impl RegisterValue {
    pub fn new(name: impl Into<String>, bits: u64, width_bits: u8) -> Self {
        Self {
            name: name.into(),
            bits,
            width_bits,
        }
    }

    pub fn masked_bits(&self) -> u64 {
        if self.width_bits >= 64 {
            self.bits
        } else {
            let mask = (1u64 << self.width_bits).wrapping_sub(1);
            self.bits & mask
        }
    }
}

/// Snapshot of registers for one thread at one acquisition time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisterSnapshot {
    pub architecture: RegisterArchitecture,
    /// OS thread id when known; otherwise unavailable as a separate field.
    pub thread_id: PlatformResult<u64>,
    pub registers: Vec<RegisterValue>,
}

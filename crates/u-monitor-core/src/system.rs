//! Identity of the binary's target; not a physical hardware discovery API.
use std::fmt;

/// Operating system targeted by the running binary, without its version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OperatingSystem {
    Windows,
    Linux,
    MacOs,
    Unknown(String),
}

impl From<&str> for OperatingSystem {
    fn from(value: &str) -> Self {
        match value {
            "windows" => Self::Windows,
            "linux" => Self::Linux,
            "macos" => Self::MacOs,
            other => Self::Unknown(other.to_owned()),
        }
    }
}

impl fmt::Display for OperatingSystem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Windows => f.write_str("Windows"),
            Self::Linux => f.write_str("Linux"),
            Self::MacOs => f.write_str("macOS"),
            Self::Unknown(value) => write!(f, "Unknown ({value})"),
        }
    }
}

/// Process target architecture. May differ from the host CPU under emulation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Architecture {
    X86_64,
    Aarch64,
    Unknown(String),
}

impl From<&str> for Architecture {
    fn from(value: &str) -> Self {
        match value {
            "x86_64" => Self::X86_64,
            "aarch64" => Self::Aarch64,
            other => Self::Unknown(other.to_owned()),
        }
    }
}

impl fmt::Display for Architecture {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::X86_64 => f.write_str("x86-64"),
            Self::Aarch64 => f.write_str("AArch64"),
            Self::Unknown(value) => write!(f, "Unknown ({value})"),
        }
    }
}

/// Build-target metadata, without claims about OS version or physical CPU.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SystemIdentity {
    pub operating_system: OperatingSystem,
    pub process_architecture: Architecture,
}

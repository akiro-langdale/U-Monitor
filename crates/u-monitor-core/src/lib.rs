//! Shared contracts independent of the UI and native operating-system adapters.

mod cpu;
mod system;

pub use cpu::CpuInfo;

pub use system::{Architecture, OperatingSystem, SystemIdentity};

use chrono::{DateTime, Utc};
use std::fmt;
use thiserror::Error;

/// The availability status of the system function/component
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Availability {
    Supported,
    PermissionRequired,
    Unavailable,
    NotImplemented,
    Error,
}

impl fmt::Display for Availability {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Availability::Supported => "SUPPORTED",
            Availability::PermissionRequired => "PERMISSION_REQUIRED",
            Availability::Unavailable => "UNAVAILABLE",
            Availability::NotImplemented => "NOT_IMPLEMENTED",
            Availability::Error => "ERROR",
        };
        write!(f, "{}", s)
    }
}

/// Origin of a value. OS-reported statistics are measured, even if the OS
/// internally estimates them. Derived values and model output must be labelled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataSource {
    /// Reported by the operating system or hardware.
    Measured,
    /// Computed from other values.
    Calculated,
    /// Produced by a simulation, never a substitute for unavailable measurements.
    Simulated,
}

/// UTC wall-clock time at which the application obtained a snapshot.
/// Not suitable for elapsed time or ordering: use `std::time::Instant` for those.
/// A timestamp does not imply an atomic snapshot across devices.
pub type Timestamp = DateTime<Utc>;

/// A failed operation, distinct from an unsupported or unimplemented capability.
/// Native error codes must be classified in the adapter, never by parsing these
/// display messages. Structured native causes will be added with native APIs.
#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum DiagnosticError {
    #[error("Platform adapter error: {0}")]
    PlatformError(String),

    #[error("Telemetry storage error: {0}")]
    StorageError(String),

    #[error("Simulation failure in model: {0}")]
    SimulationError(String),

    #[error("Data validation failed: {0}")]
    ValidationError(String),

    #[error("Unknown hardware or system error")]
    Unknown,
}

/// A value with explicit origin and acquisition time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Measurement<T> {
    pub value: T,
    pub source: DataSource,
    pub timestamp: Timestamp,
}

/// The result of a request to the platform, excluding contradictory states
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlatformResult<T> {
    /// The data has been successfully received
    Success(Measurement<T>),
    /// Additional access rights or consent are required; elevation is not automatic.
    PermissionRequired(String),
    /// Unsupported by the platform/hardware, or unavailable in current conditions.
    Unavailable(String),
    /// This metric or method has not yet been implemented for the current OS.
    NotImplemented(String),
    /// A system or hardware error occurred
    Error(DiagnosticError),
}

impl<T> PlatformResult<T> {
    /// A helper method for getting the availability status without the actual data
    pub fn availability(&self) -> Availability {
        match self {
            PlatformResult::Success(_) => Availability::Supported,
            PlatformResult::PermissionRequired(_) => Availability::PermissionRequired,
            PlatformResult::Unavailable(_) => Availability::Unavailable,
            PlatformResult::NotImplemented(_) => Availability::NotImplemented,
            PlatformResult::Error(_) => Availability::Error,
        }
    }
}

/// Per enumeration of platform subsystems or functions, the availability of which can be checked
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlatformFeature {
    SystemIdentity,
    CpuDiscovery,
    CpuMetrics,
    RamMetrics,
    DiskStorage,
    HardwareRegisters,
}

/// The contract for platform-dependent adapters
pub trait PlatformAdapter {
    /// Discovers OS-reported CPU models and topology without sampling CPU load.
    fn cpu_info(&self) -> PlatformResult<CpuInfo> {
        PlatformResult::NotImplemented("CPU discovery is not implemented".into())
    }

    /// Returns build-target metadata, not physical hardware characteristics.
    fn system_identity(&self) -> PlatformResult<SystemIdentity> {
        PlatformResult::NotImplemented("System identification is not implemented".into())
    }

    /// Returns the name of the current adapter (for example, "Linux Native", "Windows API")
    fn name(&self) -> &'static str;

    /// Checks the current availability status of a specific platform function
    fn check_availability(&self, feature: PlatformFeature) -> Availability;
}

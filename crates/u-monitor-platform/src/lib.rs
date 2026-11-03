//! Platform adapters for system identity, CPU discovery and RAM statistics.
mod cpu;
mod ram;

use std::time::SystemTime;
use u_monitor_core::{
    Architecture, Availability, CpuInfo, DataSource, Measurement, OperatingSystem, PlatformAdapter,
    PlatformFeature, PlatformResult, RamInfo, SystemIdentity,
};

/// Adapter for metadata describing the currently executing binary.
#[derive(Debug, Default)]
pub struct NativePlatformAdapter;

impl PlatformAdapter for NativePlatformAdapter {
    fn ram_info(&self) -> PlatformResult<RamInfo> {
        ram::discover()
    }

    fn cpu_info(&self) -> PlatformResult<CpuInfo> {
        cpu::discover()
    }

    fn name(&self) -> &'static str {
        "Native Platform Adapter"
    }

    fn system_identity(&self) -> PlatformResult<SystemIdentity> {
        PlatformResult::Success(Measurement {
            value: SystemIdentity {
                operating_system: OperatingSystem::from(std::env::consts::OS),
                process_architecture: Architecture::from(std::env::consts::ARCH),
            },
            // Read build metadata; no simulation or physical CPU inference.
            source: DataSource::Measured,
            timestamp: SystemTime::now().into(),
        })
    }

    fn check_availability(&self, feature: PlatformFeature) -> Availability {
        match feature {
            PlatformFeature::SystemIdentity => Availability::Supported,
            PlatformFeature::CpuDiscovery | PlatformFeature::RamMetrics => {
                if sysinfo::IS_SUPPORTED_SYSTEM {
                    Availability::Supported
                } else {
                    Availability::Unavailable
                }
            }
            PlatformFeature::CpuMetrics
            | PlatformFeature::DiskStorage
            | PlatformFeature::HardwareRegisters => Availability::NotImplemented,
        }
    }
}

/// A stub adapter for capabilities without an implementation.
#[derive(Debug, Default)]
pub struct StubPlatformAdapter;

impl StubPlatformAdapter {
    pub fn new() -> Self {
        Self
    }
}

impl PlatformAdapter for StubPlatformAdapter {
    fn name(&self) -> &'static str {
        "Stub Platform Adapter"
    }

    fn check_availability(&self, _feature: PlatformFeature) -> Availability {
        Availability::NotImplemented
    }
}

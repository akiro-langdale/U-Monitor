//! Platform adapters for system identity, CPU, RAM, memory map and mounted volumes.
mod cache;
mod cpu;
mod cpu_metrics;
mod ram;
mod registers;
mod storage;
mod virtual_memory;

/// Backend minimum between CPU counter refreshes for the native sampler.
pub const MINIMUM_CPU_SAMPLING_INTERVAL: std::time::Duration = cpu_metrics::MINIMUM_INTERVAL;
/// Upper bound for one current-process memory sample returned to the inspector.
pub use virtual_memory::MAX_MEMORY_SAMPLE_BYTES;

use std::time::SystemTime;
use u_monitor_core::{
    Architecture, Availability, CacheTopology, CpuInfo, CpuMetrics, DataSource, Measurement,
    MemoryBytes, MemoryReadRequest, OperatingSystem, PlatformAdapter, PlatformFeature,
    PlatformResult, RamInfo, RegisterSnapshot, StorageVolume, SystemIdentity, VirtualMemoryRegion,
};

/// Stateful CPU and RAM sampling adapter owned exclusively by a collector.
/// System counters are initialized lazily on the sampling thread.
#[derive(Default)]
pub struct NativeTelemetryAdapter {
    cpu: cpu_metrics::CpuSampler,
}

impl PlatformAdapter for NativeTelemetryAdapter {
    fn cpu_metrics(&mut self) -> PlatformResult<CpuMetrics> {
        self.cpu.sample()
    }

    fn ram_info(&self) -> PlatformResult<RamInfo> {
        ram::discover()
    }

    fn name(&self) -> &'static str {
        "Native Telemetry Adapter"
    }

    fn check_availability(&self, feature: PlatformFeature) -> Availability {
        match feature {
            PlatformFeature::CpuMetrics | PlatformFeature::RamMetrics => {
                if sysinfo::IS_SUPPORTED_SYSTEM {
                    Availability::Supported
                } else {
                    Availability::Unavailable
                }
            }
            _ => Availability::NotImplemented,
        }
    }
}

/// Adapter for metadata describing the currently executing binary.
#[derive(Debug, Default)]
pub struct NativePlatformAdapter;

impl PlatformAdapter for NativePlatformAdapter {
    fn storage_volumes(&self) -> PlatformResult<Vec<StorageVolume>> {
        storage::discover()
    }

    fn ram_info(&self) -> PlatformResult<RamInfo> {
        ram::discover()
    }

    fn virtual_memory_regions(&self) -> PlatformResult<Vec<VirtualMemoryRegion>> {
        virtual_memory::discover_regions()
    }

    fn read_current_process_bytes(
        &self,
        request: MemoryReadRequest,
    ) -> PlatformResult<MemoryBytes> {
        virtual_memory::read_bytes(request)
    }

    fn cpu_info(&self) -> PlatformResult<CpuInfo> {
        cpu::discover()
    }

    fn current_thread_registers(&self) -> PlatformResult<RegisterSnapshot> {
        registers::capture_current_thread()
    }

    fn cache_topology(&self) -> PlatformResult<CacheTopology> {
        cache::discover()
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
            PlatformFeature::CpuDiscovery
            | PlatformFeature::RamMetrics
            | PlatformFeature::StorageDiscovery => {
                if sysinfo::IS_SUPPORTED_SYSTEM {
                    Availability::Supported
                } else {
                    Availability::Unavailable
                }
            }
            #[cfg(any(windows, target_os = "linux"))]
            PlatformFeature::VirtualMemoryMap | PlatformFeature::MemoryBytes => {
                Availability::Supported
            }
            #[cfg(not(any(windows, target_os = "linux")))]
            PlatformFeature::VirtualMemoryMap | PlatformFeature::MemoryBytes => {
                Availability::NotImplemented
            }
            #[cfg(all(windows, target_arch = "x86_64"))]
            PlatformFeature::CurrentThreadRegisters => Availability::Supported,
            #[cfg(not(all(windows, target_arch = "x86_64")))]
            PlatformFeature::CurrentThreadRegisters => Availability::NotImplemented,
            #[cfg(any(windows, target_os = "linux"))]
            PlatformFeature::CacheTopology => Availability::Supported,
            #[cfg(not(any(windows, target_os = "linux")))]
            PlatformFeature::CacheTopology => Availability::NotImplemented,
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

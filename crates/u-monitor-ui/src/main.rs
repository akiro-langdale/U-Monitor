use u_monitor_core::{DataSource, PlatformAdapter, PlatformFeature, PlatformResult};
use u_monitor_platform::NativePlatformAdapter;

fn main() {
    let adapter = NativePlatformAdapter;
    println!("U-Monitor {}", env!("CARGO_PKG_VERSION"));
    println!("Adapter: {}", adapter.name());
    let result = adapter.system_identity();
    println!("System identity: {}", result.availability());
    match result {
        PlatformResult::Success(measurement) => {
            println!(
                "Operating system (build target): {}",
                measurement.value.operating_system
            );
            println!(
                "Process architecture: {}",
                measurement.value.process_architecture
            );
            let source = match measurement.source {
                DataSource::Measured => "MEASURED",
                DataSource::Calculated => "CALCULATED",
                DataSource::Simulated => "SIMULATED",
            };
            println!("Source: {source} (build metadata)");
            println!("Captured at: {}", measurement.timestamp.to_rfc3339());
            println!("OS version and physical CPU architecture are not detected.");
        }
        PlatformResult::PermissionRequired(reason) => eprintln!("Permission required: {reason}"),
        PlatformResult::Unavailable(reason) => eprintln!("Unavailable: {reason}"),
        PlatformResult::NotImplemented(reason) => eprintln!("Not implemented: {reason}"),
        PlatformResult::Error(error) => eprintln!("System identification failed: {error}"),
    }
    for (label, feature) in [
        ("CPU", PlatformFeature::CpuMetrics),
        ("RAM", PlatformFeature::RamMetrics),
        ("Storage", PlatformFeature::DiskStorage),
        ("Registers", PlatformFeature::HardwareRegisters),
    ] {
        println!("{label}: {}", adapter.check_availability(feature));
    }
}

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
    print_cpu(&adapter);
    for (label, feature) in [
        ("CPU load/frequency", PlatformFeature::CpuMetrics),
        ("RAM", PlatformFeature::RamMetrics),
        ("Storage", PlatformFeature::DiskStorage),
        ("Registers", PlatformFeature::HardwareRegisters),
    ] {
        println!("{label}: {}", adapter.check_availability(feature));
    }
}

fn print_cpu(adapter: &impl PlatformAdapter) {
    let result = adapter.cpu_info();
    println!("CPU discovery: {}", result.availability());
    match result {
        PlatformResult::Success(snapshot) => {
            let cpu = snapshot.value;
            let models = match cpu.model_names {
                PlatformResult::Success(measurement) => {
                    PlatformResult::Success(u_monitor_core::Measurement {
                        value: measurement.value.join("; "),
                        source: measurement.source,
                        timestamp: measurement.timestamp,
                    })
                }
                PlatformResult::PermissionRequired(reason) => {
                    PlatformResult::PermissionRequired(reason)
                }
                PlatformResult::Unavailable(reason) => PlatformResult::Unavailable(reason),
                PlatformResult::NotImplemented(reason) => PlatformResult::NotImplemented(reason),
                PlatformResult::Error(error) => PlatformResult::Error(error),
            };
            print_field("CPU model(s)", models);
            print_field("Logical processors (OS-reported)", cpu.logical_processors);
            print_field("Physical cores (OS-reported)", cpu.physical_cores);
            println!("CPU topology may reflect a VM or container rather than the physical host.");
        }
        PlatformResult::PermissionRequired(reason) => {
            eprintln!("CPU permission required: {reason}")
        }
        PlatformResult::Unavailable(reason) => eprintln!("CPU unavailable: {reason}"),
        PlatformResult::NotImplemented(reason) => eprintln!("CPU not implemented: {reason}"),
        PlatformResult::Error(error) => eprintln!("CPU discovery failed: {error}"),
    }
}

fn print_field<T: std::fmt::Display>(label: &str, result: PlatformResult<T>) {
    match result {
        PlatformResult::Success(measurement) => println!(
            "{label}: {} [source={}, captured_at={}]",
            measurement.value,
            source_label(measurement.source),
            measurement.timestamp.to_rfc3339()
        ),
        PlatformResult::PermissionRequired(reason) => {
            println!("{label}: PERMISSION_REQUIRED ({reason})")
        }
        PlatformResult::Unavailable(reason) => println!("{label}: UNAVAILABLE ({reason})"),
        PlatformResult::NotImplemented(reason) => println!("{label}: NOT_IMPLEMENTED ({reason})"),
        PlatformResult::Error(error) => println!("{label}: ERROR ({error})"),
    }
}

fn source_label(source: DataSource) -> &'static str {
    match source {
        DataSource::Measured => "MEASURED",
        DataSource::Calculated => "CALCULATED",
        DataSource::Simulated => "SIMULATED",
    }
}

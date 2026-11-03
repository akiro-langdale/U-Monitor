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
    print_ram(&adapter);
    print_storage(&adapter);
    for (label, feature) in [
        ("CPU load/frequency", PlatformFeature::CpuMetrics),
        ("Storage I/O/raw blocks", PlatformFeature::DiskStorage),
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
    print_field_with(label, result, |value| value.to_string());
}

fn print_field_with<T>(label: &str, result: PlatformResult<T>, format: impl FnOnce(&T) -> String) {
    match result {
        PlatformResult::Success(measurement) => println!(
            "{label}: {} [source={}, captured_at={}]",
            format(&measurement.value),
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

fn print_ram(adapter: &impl PlatformAdapter) {
    let result = adapter.ram_info();
    println!("RAM statistics: {}", result.availability());
    match result {
        PlatformResult::Success(snapshot) => {
            print_field_with(
                "Total RAM (OS-visible)",
                snapshot.value.total_bytes,
                format_bytes,
            );
            print_field_with(
                "Available RAM (sysinfo-reported)",
                snapshot.value.available_bytes,
                format_bytes,
            );
            print_field_with(
                "Used RAM (total - available)",
                snapshot.value.used_bytes,
                format_bytes,
            );
            println!("Available RAM may include reclaimable caches or backend estimates.");
            println!(
                "Total is OS-visible capacity, not installed DIMM capacity or a container quota."
            );
        }
        PlatformResult::PermissionRequired(reason) => {
            eprintln!("RAM permission required: {reason}")
        }
        PlatformResult::Unavailable(reason) => eprintln!("RAM unavailable: {reason}"),
        PlatformResult::NotImplemented(reason) => eprintln!("RAM not implemented: {reason}"),
        PlatformResult::Error(error) => eprintln!("RAM statistics failed: {error}"),
    }
}

fn format_bytes(bytes: &u64) -> String {
    format!(
        "{bytes} bytes (~{:.2} GiB)",
        *bytes as f64 / 1_073_741_824.0
    )
}

fn print_storage(adapter: &impl PlatformAdapter) {
    let result = adapter.storage_volumes();
    println!("Mounted-volume discovery: {}", result.availability());
    match result {
        PlatformResult::Success(snapshot) => {
            println!(
                "Reported volumes: {} [source={}, captured_at={}]",
                snapshot.value.len(),
                source_label(snapshot.source),
                snapshot.timestamp.to_rfc3339()
            );
            println!("This is not a physical-device inventory; do not sum volume capacities.");
            println!(
                "Linux tmpfs and network mounts are excluded by the current backend features."
            );
            for volume in snapshot.value {
                // Debug formatting escapes native paths instead of silently replacing invalid UTF-8.
                println!("Mount point: {:?}", volume.mount_point);
                print_field_with("Volume name", volume.name, |text| format!("{text:?}"));
                print_field_with("File system", volume.file_system, |text| {
                    format!("{text:?}")
                });
                println!("Media type (backend-reported): {}", volume.kind);
                print_field_with("Total volume space", volume.space.total_bytes, format_bytes);
                print_field_with(
                    "Available volume space",
                    volume.space.available_bytes,
                    format_bytes,
                );
                print_field_with(
                    "Used/unavailable (total - available)",
                    volume.space.used_bytes,
                    format_bytes,
                );
            }
            println!("Used/unavailable space may include reserved space and quota restrictions.");
        }
        PlatformResult::PermissionRequired(reason) => {
            eprintln!("Storage permission required: {reason}")
        }
        PlatformResult::Unavailable(reason) => eprintln!("Storage unavailable: {reason}"),
        PlatformResult::NotImplemented(reason) => eprintln!("Storage not implemented: {reason}"),
        PlatformResult::Error(error) => eprintln!("Storage discovery failed: {error}"),
    }
}

#[cfg(test)]
mod tests {
    use super::format_bytes;

    #[test]
    fn byte_display_uses_binary_gib_and_preserves_exact_bytes() {
        assert_eq!(format_bytes(&0), "0 bytes (~0.00 GiB)");
        assert_eq!(format_bytes(&1_073_741_824), "1073741824 bytes (~1.00 GiB)");
        assert_eq!(format_bytes(&1_610_612_736), "1610612736 bytes (~1.50 GiB)");
        assert!(format_bytes(&u64::MAX).starts_with("18446744073709551615 bytes"));
    }
}

use u_monitor_core::{Architecture, OperatingSystem};

#[test]
fn known_operating_systems_have_readable_labels() {
    for (raw, expected, label) in [
        ("windows", OperatingSystem::Windows, "Windows"),
        ("linux", OperatingSystem::Linux, "Linux"),
        ("macos", OperatingSystem::MacOs, "macOS"),
    ] {
        let value = OperatingSystem::from(raw);
        assert_eq!(value, expected);
        assert_eq!(value.to_string(), label);
    }
}

#[test]
fn known_process_architectures_have_readable_labels() {
    for (raw, expected, label) in [
        ("x86_64", Architecture::X86_64, "x86-64"),
        ("aarch64", Architecture::Aarch64, "AArch64"),
    ] {
        let value = Architecture::from(raw);
        assert_eq!(value, expected);
        assert_eq!(value.to_string(), label);
    }
}

#[test]
fn unknown_targets_are_preserved_without_guessing() {
    for raw in ["freebsd", "future-os", ""] {
        assert_eq!(
            OperatingSystem::from(raw),
            OperatingSystem::Unknown(raw.into())
        );
        assert_eq!(
            OperatingSystem::from(raw).to_string(),
            format!("Unknown ({raw})")
        );
    }
    for raw in ["riscv64", "x86", ""] {
        assert_eq!(Architecture::from(raw), Architecture::Unknown(raw.into()));
        assert_eq!(
            Architecture::from(raw).to_string(),
            format!("Unknown ({raw})")
        );
    }
}

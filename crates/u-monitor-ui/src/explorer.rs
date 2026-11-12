//! Cross-panel selection and provenance JSON export.

use std::path::PathBuf;
use u_monitor_core::{
    Availability, CacheTopology, CpuInfo, PlatformFeature, PlatformResult, RegisterSnapshot,
    StorageVolume, SystemIdentity,
};

#[derive(Debug, Clone, Default)]
pub(crate) struct ExplorerSelection {
    pub logical_cpu: Option<usize>,
    pub cache_node: Option<usize>,
    pub sim_address: Option<u64>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct ExplorerState {
    pub selection: ExplorerSelection,
    pub last_export_path: Option<PathBuf>,
    pub last_export_error: Option<String>,
}

impl ExplorerState {
    pub(crate) fn select_logical_cpu(
        &mut self,
        index: usize,
        topology: &PlatformResult<CacheTopology>,
    ) {
        self.selection.logical_cpu = Some(index);
        self.selection.cache_node = match topology {
            PlatformResult::Success(measurement) => measurement
                .value
                .nodes
                .iter()
                .position(|node| node.shared_logical_processors.contains(&index)),
            _ => None,
        };
        self.selection.sim_address = Some(0x1000 + (index as u64) * 0x40);
    }
}

/// Builds a compact provenance JSON document from current panel snapshots.
pub(crate) fn build_snapshot_json(
    identity: &PlatformResult<SystemIdentity>,
    capabilities: &[(PlatformFeature, Availability)],
    cpu: &PlatformResult<CpuInfo>,
    storage: &PlatformResult<Vec<StorageVolume>>,
    cache: &PlatformResult<CacheTopology>,
    registers: &PlatformResult<RegisterSnapshot>,
    selection: &ExplorerSelection,
) -> String {
    let mut out = String::from("{\n");
    out.push_str("  \"app\": \"u-monitor\",\n");
    out.push_str(&format!(
        "  \"version\": \"{}\",\n",
        env!("CARGO_PKG_VERSION")
    ));
    out.push_str(&format!(
        "  \"selection\": {{ \"logical_cpu\": {}, \"cache_node\": {}, \"sim_address\": {} }},\n",
        json_opt_usize(selection.logical_cpu),
        json_opt_usize(selection.cache_node),
        json_opt_u64(selection.sim_address)
    ));
    out.push_str("  \"capabilities\": {\n");
    for (index, (feature, availability)) in capabilities.iter().enumerate() {
        let comma = if index + 1 == capabilities.len() {
            ""
        } else {
            ","
        };
        out.push_str(&format!(
            "    \"{}\": \"{}\"{comma}\n",
            feature, availability
        ));
    }
    out.push_str("  },\n");
    out.push_str(&format!(
        "  \"identity\": {},\n",
        availability_json(identity)
    ));
    out.push_str(&format!("  \"cpu\": {},\n", availability_json(cpu)));
    out.push_str(&format!("  \"storage\": {},\n", availability_json(storage)));
    out.push_str(&format!("  \"cache\": {},\n", availability_json(cache)));
    out.push_str(&format!(
        "  \"registers\": {}\n",
        availability_json(registers)
    ));
    out.push('}');
    out
}

fn availability_json<T>(result: &PlatformResult<T>) -> String {
    match result {
        PlatformResult::Success(measurement) => format!(
            "{{ \"availability\": \"SUPPORTED\", \"source\": \"{}\", \"timestamp\": \"{}\" }}",
            match measurement.source {
                u_monitor_core::DataSource::Measured => "MEASURED",
                u_monitor_core::DataSource::Calculated => "CALCULATED",
                u_monitor_core::DataSource::Simulated => "SIMULATED",
            },
            measurement.timestamp.to_rfc3339()
        ),
        other => format!("{{ \"availability\": \"{}\" }}", other.availability()),
    }
}

fn json_opt_usize(value: Option<usize>) -> String {
    value
        .map(|v| v.to_string())
        .unwrap_or_else(|| "null".into())
}

fn json_opt_u64(value: Option<u64>) -> String {
    value
        .map(|v| format!("{v}"))
        .unwrap_or_else(|| "null".into())
}

pub(crate) fn export_snapshot_to_temp(json: &str) -> Result<PathBuf, String> {
    let path = std::env::temp_dir().join(format!(
        "u-monitor-snapshot-{}.json",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    ));
    std::fs::write(&path, json).map_err(|error| format!("write {}: {error}", path.display()))?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_json_includes_selection_and_availability() {
        let json = build_snapshot_json(
            &PlatformResult::NotImplemented("x".into()),
            &[(PlatformFeature::SystemIdentity, Availability::Supported)],
            &PlatformResult::Unavailable("cpu".into()),
            &PlatformResult::NotImplemented("storage".into()),
            &PlatformResult::NotImplemented("cache".into()),
            &PlatformResult::NotImplemented("regs".into()),
            &ExplorerSelection {
                logical_cpu: Some(2),
                cache_node: Some(0),
                sim_address: Some(0x1080),
            },
        );
        assert!(json.contains("\"logical_cpu\": 2"));
        assert!(json.contains("\"SystemIdentity\": \"SUPPORTED\""));
        assert!(json.contains("\"availability\": \"UNAVAILABLE\""));
    }
}

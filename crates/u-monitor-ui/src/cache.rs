//! Cache hierarchy metadata panel. Never displays reconstructed cache-line bytes.
use eframe::egui;
use u_monitor_core::{CacheCounters, CacheTopology, DataSource, PlatformResult};

pub(crate) fn show(
    ui: &mut egui::Ui,
    topology: &PlatformResult<CacheTopology>,
    counters: &mut PlatformResult<CacheCounters>,
    refresh_counters: &dyn Fn() -> PlatformResult<CacheCounters>,
) {
    ui.label(
        "Показаны только характеристики иерархии кэша (уровень, размер, линия, ассоциативность, разделение). Содержимое кэш-линий недоступно и не симулируется здесь.",
    );
    ui.add_space(8.0);
    ui.monospace(topology.availability().to_string());
    match topology {
        PlatformResult::Success(measurement) => {
            ui.monospace(match measurement.source {
                DataSource::Measured => "MEASURED",
                DataSource::Calculated => "CALCULATED",
                DataSource::Simulated => "SIMULATED",
            });
            ui.small(format!("UTC {}", measurement.timestamp.to_rfc3339()));
            ui.label(format!("Узлов: {}", measurement.value.nodes.len()));
            egui::ScrollArea::both()
                .id_salt("cache_topology")
                .show(ui, |ui| {
                    egui::Grid::new("cache_grid")
                        .striped(true)
                        .spacing([14.0, 6.0])
                        .show(ui, |ui| {
                            ui.label("Уровень");
                            ui.label("Тип");
                            ui.label("Размер");
                            ui.label("Линия");
                            ui.label("Ассоц.");
                            ui.label("Логические CPU");
                            ui.end_row();
                            for node in &measurement.value.nodes {
                                ui.monospace(format!("L{}", node.level));
                                ui.monospace(node.kind.to_string());
                                ui.label(match node.size_bytes {
                                    Some(bytes) => format_bytes(bytes),
                                    None => "UNAVAILABLE".into(),
                                });
                                ui.label(match node.line_size_bytes {
                                    Some(bytes) => format!("{bytes} B"),
                                    None => "UNAVAILABLE".into(),
                                });
                                ui.label(if node.fully_associative {
                                    "fully".into()
                                } else {
                                    match node.associativity {
                                        Some(ways) => format!("{ways}-way"),
                                        None => "UNAVAILABLE".into(),
                                    }
                                });
                                ui.monospace(format_cpus(&node.shared_logical_processors));
                                ui.end_row();
                            }
                        });
                });
        }
        PlatformResult::PermissionRequired(r)
        | PlatformResult::Unavailable(r)
        | PlatformResult::NotImplemented(r) => {
            ui.label(r);
        }
        PlatformResult::Error(e) => {
            ui.colored_label(ui.visuals().error_fg_color, e.to_string());
        }
    }

    ui.add_space(16.0);
    ui.heading("Счётчики кэша (hardware)");
    ui.label(
        "Короткое окно PMC/perf для references/misses текущего процесса. SIMULATED значения не подставляются.",
    );
    if ui.button("Сэмплировать счётчики").clicked() {
        *counters = refresh_counters();
    }
    ui.add_space(4.0);
    ui.monospace(counters.availability().to_string());
    match counters {
        PlatformResult::Success(measurement) => {
            ui.monospace(match measurement.source {
                DataSource::Measured => "MEASURED",
                DataSource::Calculated => "CALCULATED",
                DataSource::Simulated => "SIMULATED",
            });
            ui.small(format!("UTC {}", measurement.timestamp.to_rfc3339()));
            egui::Grid::new("cache_counters_grid")
                .striped(true)
                .spacing([14.0, 6.0])
                .show(ui, |ui| {
                    ui.label("References");
                    ui.monospace(format_counter(&measurement.value.references));
                    ui.end_row();
                    ui.label("Misses");
                    ui.monospace(format_counter(&measurement.value.misses));
                    ui.end_row();
                    ui.label("Window");
                    ui.monospace(match &measurement.value.window_nanos {
                        PlatformResult::Success(window) => {
                            format!("{:.1} ms", window.value as f64 / 1_000_000.0)
                        }
                        other => other.availability().to_string(),
                    });
                    ui.end_row();
                });
        }
        PlatformResult::PermissionRequired(r)
        | PlatformResult::Unavailable(r)
        | PlatformResult::NotImplemented(r) => {
            ui.label(r.as_str());
        }
        PlatformResult::Error(e) => {
            ui.colored_label(ui.visuals().error_fg_color, e.to_string());
        }
    }
}

fn format_counter(value: &PlatformResult<u64>) -> String {
    match value {
        PlatformResult::Success(measurement) => measurement.value.to_string(),
        other => other.availability().to_string(),
    }
}

fn format_bytes(bytes: u64) -> String {
    if bytes >= 1_048_576 {
        format!("{:.2} MiB", bytes as f64 / 1_048_576.0)
    } else if bytes >= 1024 {
        format!("{:.2} KiB", bytes as f64 / 1024.0)
    } else {
        format!("{bytes} B")
    }
}

fn format_cpus(cpus: &[usize]) -> String {
    if cpus.is_empty() {
        return "—".into();
    }
    if cpus.len() <= 8 {
        return cpus
            .iter()
            .map(|cpu| cpu.to_string())
            .collect::<Vec<_>>()
            .join(",");
    }
    format!("{}…(+{})", cpus[0], cpus.len().saturating_sub(1))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::UNIX_EPOCH;
    use u_monitor_core::{CacheKind, CacheNode, Measurement};

    #[test]
    fn panel_renders_topology_and_missing_fields() {
        let ctx = egui::Context::default();
        let topology = PlatformResult::Success(Measurement {
            value: CacheTopology {
                nodes: vec![CacheNode {
                    level: 1,
                    kind: CacheKind::Data,
                    size_bytes: Some(32 * 1024),
                    line_size_bytes: Some(64),
                    associativity: Some(8),
                    fully_associative: false,
                    shared_logical_processors: vec![0, 1],
                }],
            },
            source: DataSource::Measured,
            timestamp: UNIX_EPOCH.into(),
        });
        let mut counters = PlatformResult::Unavailable("not sampled".into());
        let output = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                show(ui, &topology, &mut counters, &|| {
                    PlatformResult::NotImplemented("unused".into())
                })
            });
        });
        assert!(!output.shapes.is_empty());
    }
}

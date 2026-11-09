//! Memory-hierarchy simulation panel. All values shown here are SIMULATED.
use eframe::egui;
use std::time::{Duration, Instant};
use u_monitor_simulation::{
    AccessResult, DEFAULT_DEMO_ADDRESSES, HierarchySimulator, HierarchyStats, MemoryLevel,
};

pub(crate) struct SimulationPanel {
    simulator: HierarchySimulator,
    last: Option<AccessResult>,
    stats: HierarchyStats,
    demo_index: usize,
    custom_address: String,
    playback: Option<Playback>,
}

struct Playback {
    result: AccessResult,
    started: Instant,
    step_ms: u64,
}

impl SimulationPanel {
    pub(crate) fn new() -> Self {
        let simulator = HierarchySimulator::new_default();
        Self {
            stats: simulator.stats().value,
            simulator,
            last: None,
            demo_index: 0,
            custom_address: "0x1000".into(),
            playback: None,
        }
    }

    pub(crate) fn show(&mut self, ui: &mut egui::Ui) {
        ui.label(
            "Эта панель запускает учебную модель L1→L2→RAM (set-associative LRU). Все latency, hit/miss и трассировки помечены SIMULATED и не смешиваются с MEASURED данными ОС.",
        );
        ui.add_space(8.0);
        ui.monospace("SIMULATED");

        let config = self.simulator.config();
        ui.add_space(8.0);
        egui::Grid::new("sim_config")
            .spacing([16.0, 6.0])
            .show(ui, |ui| {
                ui.label("L1");
                ui.monospace(format!(
                    "{} sets × {}-way, line {} B, hit {} ns",
                    config.l1.sets,
                    config.l1.ways,
                    config.l1.line_size_bytes,
                    config.l1.hit_latency_ns
                ));
                ui.end_row();
                ui.label("L2");
                ui.monospace(format!(
                    "{} sets × {}-way, line {} B, hit {} ns",
                    config.l2.sets,
                    config.l2.ways,
                    config.l2.line_size_bytes,
                    config.l2.hit_latency_ns
                ));
                ui.end_row();
                ui.label("RAM");
                ui.monospace(format!("{} ns (model)", config.ram_latency_ns));
                ui.end_row();
            });

        ui.add_space(12.0);
        ui.horizontal(|ui| {
            if ui.button("Демо-доступ").clicked() {
                let address =
                    DEFAULT_DEMO_ADDRESSES[self.demo_index % DEFAULT_DEMO_ADDRESSES.len()];
                self.demo_index = self.demo_index.wrapping_add(1);
                self.run_access(address);
            }
            if ui.button("Сбросить модель").clicked() {
                self.simulator.reset();
                self.stats = self.simulator.stats().value;
                self.last = None;
                self.playback = None;
                self.demo_index = 0;
            }
        });

        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.label("Адрес");
            ui.text_edit_singleline(&mut self.custom_address);
            if ui.button("Доступ").clicked()
                && let Some(address) = parse_address(&self.custom_address)
            {
                self.run_access(address);
            }
        });
        if parse_address(&self.custom_address).is_none() {
            ui.colored_label(
                ui.visuals().warn_fg_color,
                "Ожидается hex (0x1000) или десятичное число.",
            );
        }

        ui.add_space(16.0);
        self.show_hierarchy(ui);
        ui.add_space(16.0);
        self.show_stats(ui);
        ui.add_space(12.0);
        self.show_trace(ui);

        if self.playback.is_some() {
            ui.ctx().request_repaint_after(Duration::from_millis(32));
        }
    }

    fn run_access(&mut self, address: u64) {
        let measurement = self.simulator.access(address);
        debug_assert_eq!(measurement.source, u_monitor_core::DataSource::Simulated);
        self.stats = self.simulator.stats().value;
        self.last = Some(measurement.value.clone());
        self.playback = Some(Playback {
            result: measurement.value,
            started: Instant::now(),
            step_ms: 450,
        });
    }

    fn active_step_index(&self) -> Option<usize> {
        let playback = self.playback.as_ref()?;
        if playback.result.steps.is_empty() {
            return None;
        }
        let elapsed = playback.started.elapsed().as_millis() as u64;
        let index = (elapsed / playback.step_ms) as usize;
        Some(index.min(playback.result.steps.len() - 1))
    }

    fn show_hierarchy(&mut self, ui: &mut egui::Ui) {
        let active_level = self.active_step_index().and_then(|index| {
            self.playback
                .as_ref()
                .and_then(|playback| playback.result.steps.get(index).map(|step| step.level))
        });
        ui.heading("Путь доступа");
        ui.horizontal(|ui| {
            level_box(ui, "CPU", false);
            ui.label("→");
            level_box(ui, "L1", active_level == Some(MemoryLevel::L1));
            ui.label("→");
            level_box(ui, "L2", active_level == Some(MemoryLevel::L2));
            ui.label("→");
            level_box(ui, "RAM", active_level == Some(MemoryLevel::Ram));
        });
        if let Some(result) = &self.last {
            ui.add_space(6.0);
            ui.label(format!(
                "Последний доступ: {:#x} → линия {:#x} · {} · latency {} ns (SIMULATED)",
                result.address,
                result.line_address,
                if result.hit {
                    format!("HIT в {}", result.resolved_at.label())
                } else {
                    format!("разрешён в {}", result.resolved_at.label())
                },
                result.latency_ns
            ));
        }
    }

    fn show_stats(&self, ui: &mut egui::Ui) {
        ui.heading("Счётчики модели");
        egui::Grid::new("sim_stats")
            .spacing([20.0, 4.0])
            .show(ui, |ui| {
                ui.label("Доступов");
                ui.monospace(self.stats.accesses.to_string());
                ui.end_row();
                ui.label("L1 hits");
                ui.monospace(self.stats.l1_hits.to_string());
                ui.end_row();
                ui.label("L2 hits");
                ui.monospace(self.stats.l2_hits.to_string());
                ui.end_row();
                ui.label("RAM fills");
                ui.monospace(self.stats.ram_fills.to_string());
                ui.end_row();
                ui.label("Evictions");
                ui.monospace(self.stats.evictions.to_string());
                ui.end_row();
                ui.label("Σ latency");
                ui.monospace(format!("{} ns", self.stats.total_latency_ns));
                ui.end_row();
            });
    }

    fn show_trace(&self, ui: &mut egui::Ui) {
        ui.heading("Трассировка");
        let Some(playback) = &self.playback else {
            ui.label("Нажмите «Демо-доступ» или «Доступ», чтобы увидеть шаги модели.");
            return;
        };
        let active = self.active_step_index();
        egui::ScrollArea::vertical()
            .id_salt("sim_trace")
            .max_height(220.0)
            .show(ui, |ui| {
                for (index, step) in playback.result.steps.iter().enumerate() {
                    let label = format!(
                        "{:>2}. {} {}",
                        index + 1,
                        step.level.label(),
                        step.event.label()
                    );
                    if Some(index) == active {
                        ui.colored_label(ui.visuals().strong_text_color(), label);
                    } else {
                        ui.monospace(label);
                    }
                }
            });
    }
}

fn level_box(ui: &mut egui::Ui, label: &str, active: bool) {
    let fill = if active {
        ui.visuals().selection.bg_fill
    } else {
        ui.visuals().widgets.inactive.bg_fill
    };
    egui::Frame::NONE
        .fill(fill)
        .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
        .inner_margin(egui::Margin::symmetric(10, 8))
        .show(ui, |ui| {
            ui.monospace(label);
        });
}

fn parse_address(text: &str) -> Option<u64> {
    let text = text.trim();
    if let Some(hex) = text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")) {
        u64::from_str_radix(hex, 16).ok()
    } else {
        text.parse::<u64>().ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_hex_and_decimal_addresses() {
        assert_eq!(parse_address("0x1000"), Some(0x1000));
        assert_eq!(parse_address("4096"), Some(4096));
        assert!(parse_address("nope").is_none());
    }

    #[test]
    fn panel_renders_initial_simulated_state() {
        let ctx = egui::Context::default();
        let mut panel = SimulationPanel::new();
        let output = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| panel.show(ui));
        });
        assert!(!output.shapes.is_empty());
        assert_eq!(panel.stats.accesses, 0);
    }
}

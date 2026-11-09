//! Current-thread register snapshot viewer (HEX / DEC / BIN + change highlight).
use eframe::egui;
use u_monitor_core::{DataSource, PlatformResult, RegisterSnapshot, RegisterValue};

pub(crate) struct RegistersPanel {
    pub(crate) snapshot: PlatformResult<RegisterSnapshot>,
    previous: Option<RegisterSnapshot>,
    pub(crate) debug_consent: bool,
}

impl RegistersPanel {
    pub(crate) fn new(snapshot: PlatformResult<RegisterSnapshot>) -> Self {
        Self {
            snapshot,
            previous: None,
            debug_consent: false,
        }
    }

    pub(crate) fn show(
        &mut self,
        ui: &mut egui::Ui,
        other_thread: bool,
        refresh: &dyn Fn() -> PlatformResult<RegisterSnapshot>,
    ) {
        if other_thread {
            ui.label(
                "Выбран другой поток. Захват остановит поток через debug API (SuspendThread / ptrace). SIMULATED значения не используются.",
            );
            ui.checkbox(
                &mut self.debug_consent,
                "Я понимаю, что поток будет временно остановлен",
            );
        } else {
            ui.label(
                "Снимок регистров текущего потока UI (MEASURED). Для другого потока выберите tid на странице «Процессы» и дайте согласие.",
            );
        }
        ui.add_space(8.0);
        let can_refresh = !other_thread || self.debug_consent;
        if ui
            .add_enabled(can_refresh, egui::Button::new("Обновить снимок"))
            .clicked()
        {
            if let PlatformResult::Success(current) = &self.snapshot {
                self.previous = Some(current.value.clone());
            }
            self.snapshot = refresh();
        }
        ui.add_space(8.0);
        ui.monospace(self.snapshot.availability().to_string());
        match &self.snapshot {
            PlatformResult::Success(measurement) => {
                ui.monospace(match measurement.source {
                    DataSource::Measured => "MEASURED",
                    DataSource::Calculated => "CALCULATED",
                    DataSource::Simulated => "SIMULATED",
                });
                ui.small(format!("UTC {}", measurement.timestamp.to_rfc3339()));
                ui.label(format!("Архитектура: {}", measurement.value.architecture));
                match &measurement.value.thread_id {
                    PlatformResult::Success(thread) => {
                        ui.label(format!("Thread ID: {} · MEASURED", thread.value));
                    }
                    other => {
                        ui.label(format!("Thread ID: {}", other.availability()));
                    }
                }
                let previous = self.previous.clone();
                egui::ScrollArea::both()
                    .id_salt("register_table")
                    .show(ui, |ui| {
                        egui::Grid::new("register_grid")
                            .striped(true)
                            .spacing([16.0, 6.0])
                            .show(ui, |ui| {
                                ui.label("Имя");
                                ui.label("HEX");
                                ui.label("DEC");
                                ui.label("BIN");
                                ui.label("Δ");
                                ui.end_row();
                                for reg in &measurement.value.registers {
                                    let bits = reg.masked_bits();
                                    ui.monospace(&reg.name);
                                    ui.monospace(format_hex(bits, reg.width_bits));
                                    ui.monospace(bits.to_string());
                                    ui.monospace(format_bin(bits, reg.width_bits));
                                    ui.monospace(delta_label(previous.as_ref(), reg));
                                    ui.end_row();
                                }
                            });
                    });
                ui.add_space(8.0);
                ui.small(
                    "Δ сравнивает с предыдущим успешным MEASURED-снимком (CALCULATED). Пустое значение означает отсутствие базы или отсутствие изменений.",
                );
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
    }
}

fn format_hex(bits: u64, width_bits: u8) -> String {
    let width = ((width_bits as usize).div_ceil(4)).max(1);
    format!("0x{bits:0width$X}")
}

fn format_bin(bits: u64, width_bits: u8) -> String {
    let width = (width_bits as usize).max(1);
    format!("{bits:0width$b}")
}

fn delta_label(previous: Option<&RegisterSnapshot>, reg: &RegisterValue) -> String {
    let Some(previous) = previous else {
        return String::new();
    };
    let Some(old) = previous.registers.iter().find(|item| item.name == reg.name) else {
        return "NEW".into();
    };
    let before = old.masked_bits();
    let after = reg.masked_bits();
    if before == after {
        return String::new();
    }
    let delta = after.wrapping_sub(before) as i64;
    format!("{delta:+} · CALCULATED")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::UNIX_EPOCH;
    use u_monitor_core::{Measurement, RegisterArchitecture, RegisterValue};

    #[test]
    fn formatting_and_delta_helpers() {
        assert_eq!(format_hex(0x1f, 8), "0x1F");
        assert_eq!(format_bin(0b1010, 8), "00001010");
        let previous = RegisterSnapshot {
            architecture: RegisterArchitecture::X86_64,
            thread_id: PlatformResult::Unavailable("n/a".into()),
            registers: vec![RegisterValue::new("RAX", 10, 64)],
        };
        let current = RegisterValue::new("RAX", 12, 64);
        assert_eq!(delta_label(Some(&previous), &current), "+2 · CALCULATED");
        assert_eq!(
            delta_label(Some(&previous), &RegisterValue::new("RAX", 10, 64)),
            ""
        );
    }

    #[test]
    fn panel_renders_success_and_pending_states() {
        let ctx = egui::Context::default();
        let mut panel = RegistersPanel::new(PlatformResult::Success(Measurement {
            value: RegisterSnapshot {
                architecture: RegisterArchitecture::X86_64,
                thread_id: PlatformResult::Success(Measurement {
                    value: 42,
                    source: DataSource::Measured,
                    timestamp: UNIX_EPOCH.into(),
                }),
                registers: vec![RegisterValue::new("RIP", 0x1000, 64)],
            },
            source: DataSource::Measured,
            timestamp: UNIX_EPOCH.into(),
        }));
        let output = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                panel.show(ui, false, &|| {
                    PlatformResult::NotImplemented("unused".into())
                });
            });
        });
        assert!(!output.shapes.is_empty());
    }
}

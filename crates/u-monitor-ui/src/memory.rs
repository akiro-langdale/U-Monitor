//! Current-process virtual memory map and read-only HEX samples.
use eframe::egui;
use u_monitor_core::{
    DataSource, MemoryBytes, MemoryReadRequest, PlatformResult, VirtualMemoryRegion,
};
use u_monitor_platform::MAX_MEMORY_SAMPLE_BYTES;

pub(crate) struct MemoryPanel {
    regions: PlatformResult<Vec<VirtualMemoryRegion>>,
    selected: Option<usize>,
    sample: Option<PlatformResult<MemoryBytes>>,
    filter_readable: bool,
}

impl MemoryPanel {
    pub(crate) fn new(regions: PlatformResult<Vec<VirtualMemoryRegion>>) -> Self {
        Self {
            regions,
            selected: None,
            sample: None,
            filter_readable: true,
        }
    }

    pub(crate) fn show(
        &mut self,
        ui: &mut egui::Ui,
        read_bytes: &dyn Fn(MemoryReadRequest) -> PlatformResult<MemoryBytes>,
    ) {
        ui.label(
            "Карта относится к адресному пространству текущего процесса U-Monitor. Это не дамп физической RAM и не инспекция чужих процессов.",
        );
        ui.add_space(8.0);
        ui.checkbox(
            &mut self.filter_readable,
            "Только читаемые COMMITTED-регионы",
        );
        ui.add_space(8.0);
        ui.monospace(self.regions.availability().to_string());
        match &self.regions {
            PlatformResult::Success(measurement) => {
                ui.monospace(match measurement.source {
                    DataSource::Measured => "MEASURED",
                    DataSource::Calculated => "CALCULATED",
                    DataSource::Simulated => "SIMULATED",
                });
                ui.small(format!("UTC {}", measurement.timestamp.to_rfc3339()));
                let rows: Vec<(usize, &VirtualMemoryRegion)> = measurement
                    .value
                    .iter()
                    .enumerate()
                    .filter(|(_, region)| !self.filter_readable || region.readable)
                    .collect();
                ui.label(format!(
                    "Показано {} из {} регионов",
                    rows.len(),
                    measurement.value.len()
                ));
                egui::ScrollArea::both()
                    .id_salt("memory_regions")
                    .max_height(280.0)
                    .show(ui, |ui| {
                        egui::Grid::new("memory_region_grid")
                            .striped(true)
                            .spacing([12.0, 6.0])
                            .show(ui, |ui| {
                                ui.label("База");
                                ui.label("Размер");
                                ui.label("Состояние");
                                ui.label("Доступ");
                                ui.label("Тип");
                                ui.label("");
                                ui.end_row();
                                for (index, region) in rows {
                                    ui.monospace(format!("0x{:016X}", region.base_address));
                                    ui.label(format_bytes(region.size_bytes));
                                    ui.monospace(region.state.to_string());
                                    ui.monospace(region.protection.label());
                                    ui.monospace(region.kind.to_string());
                                    if ui
                                        .add_enabled(region.readable, egui::Button::new("HEX"))
                                        .clicked()
                                    {
                                        self.selected = Some(index);
                                        let length = (region.size_bytes as usize)
                                            .clamp(1, MAX_MEMORY_SAMPLE_BYTES);
                                        self.sample = Some(read_bytes(MemoryReadRequest {
                                            address: region.base_address,
                                            length,
                                        }));
                                    }
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
        ui.heading("HEX-просмотр");
        ui.label(format!(
            "Только чтение, максимум {MAX_MEMORY_SAMPLE_BYTES} байт за запрос."
        ));
        match (&self.selected, &self.sample) {
            (Some(index), Some(sample)) => {
                ui.small(format!("Регион #{index}"));
                show_sample(ui, sample);
            }
            _ => {
                ui.label("Выберите читаемый регион кнопкой HEX.");
            }
        }
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

fn show_sample(ui: &mut egui::Ui, sample: &PlatformResult<MemoryBytes>) {
    ui.monospace(sample.availability().to_string());
    match sample {
        PlatformResult::Success(measurement) => {
            ui.monospace(match measurement.source {
                DataSource::Measured => "MEASURED",
                DataSource::Calculated => "CALCULATED",
                DataSource::Simulated => "SIMULATED",
            });
            ui.small(format!(
                "0x{:016X} · {} байт · UTC {}",
                measurement.value.address,
                measurement.value.bytes.len(),
                measurement.timestamp.to_rfc3339()
            ));
            egui::ScrollArea::vertical()
                .id_salt("memory_hex")
                .max_height(240.0)
                .show(ui, |ui| {
                    ui.monospace(hex_dump(
                        measurement.value.address,
                        &measurement.value.bytes,
                    ));
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
}

fn hex_dump(base: u64, bytes: &[u8]) -> String {
    let mut out = String::new();
    for (offset, chunk) in bytes.chunks(16).enumerate() {
        let address = base + (offset as u64 * 16);
        out.push_str(&format!("{address:016X}  "));
        for index in 0..16 {
            if let Some(byte) = chunk.get(index) {
                out.push_str(&format!("{byte:02X} "));
            } else {
                out.push_str("   ");
            }
            if index == 7 {
                out.push(' ');
            }
        }
        out.push(' ');
        for byte in chunk {
            let ch = *byte;
            out.push(if (0x20..0x7f).contains(&ch) {
                ch as char
            } else {
                '.'
            });
        }
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::UNIX_EPOCH;
    use u_monitor_core::{
        Measurement, MemoryProtection, MemoryRegionKind, MemoryState, PlatformResult,
    };

    #[test]
    fn hex_dump_keeps_address_ascii_and_padding() {
        let text = hex_dump(0x1000, &[0x41, 0x00, 0x7e, 0xff]);
        assert!(text.contains("0000000000001000"));
        assert!(text.contains("41 00 7E FF"));
        assert!(text.contains("A.~."));
    }

    #[test]
    fn panel_renders_region_list_and_sample_states() {
        let ctx = egui::Context::default();
        let mut panel = MemoryPanel::new(PlatformResult::Success(Measurement {
            value: vec![VirtualMemoryRegion {
                base_address: 0x2000,
                size_bytes: 4096,
                state: MemoryState::Committed,
                protection: MemoryProtection {
                    read: true,
                    write: false,
                    execute: false,
                    guard: false,
                },
                kind: MemoryRegionKind::Private,
                readable: true,
            }],
            source: DataSource::Measured,
            timestamp: UNIX_EPOCH.into(),
        }));
        panel.selected = Some(0);
        panel.sample = Some(PlatformResult::Success(Measurement {
            value: MemoryBytes {
                address: 0x2000,
                bytes: b"AB".to_vec(),
            },
            source: DataSource::Measured,
            timestamp: UNIX_EPOCH.into(),
        }));
        let output = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                panel.show(ui, &|_| PlatformResult::NotImplemented("unused".into()));
            });
        });
        assert!(!output.shapes.is_empty());
    }
}

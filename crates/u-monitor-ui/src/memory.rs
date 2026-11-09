//! Current-process virtual memory map and read-only HEX samples.
use eframe::egui;
use u_monitor_core::{
    DataSource, Measurement, MemoryBytes, MemoryReadRequest, PlatformResult, Timestamp,
    VirtualMemoryRegion,
};
use u_monitor_platform::MAX_MEMORY_SAMPLE_BYTES;

#[derive(Debug, Clone, PartialEq, Eq)]
struct SampleKey {
    region_index: usize,
    address: u64,
    length: usize,
}

/// Byte-level comparison of two measured samples. The diff itself is CALCULATED.
#[derive(Debug, Clone, PartialEq, Eq)]
struct HexDiff {
    address: u64,
    before: Vec<u8>,
    after: Vec<u8>,
    before_at: Timestamp,
    after_at: Timestamp,
    changed_offsets: Vec<usize>,
}

impl HexDiff {
    fn compare(
        before: &Measurement<MemoryBytes>,
        after: &Measurement<MemoryBytes>,
    ) -> Result<Self, String> {
        if before.value.address != after.value.address {
            return Err("Снимки относятся к разным адресам".into());
        }
        if before.value.bytes.len() != after.value.bytes.len() {
            return Err("Снимки имеют разную длину".into());
        }
        let changed_offsets = before
            .value
            .bytes
            .iter()
            .zip(after.value.bytes.iter())
            .enumerate()
            .filter_map(|(index, (left, right))| (left != right).then_some(index))
            .collect();
        Ok(Self {
            address: before.value.address,
            before: before.value.bytes.clone(),
            after: after.value.bytes.clone(),
            before_at: before.timestamp,
            after_at: after.timestamp,
            changed_offsets,
        })
    }
}

pub(crate) struct MemoryPanel {
    regions: PlatformResult<Vec<VirtualMemoryRegion>>,
    selected: Option<SampleKey>,
    sample: Option<PlatformResult<MemoryBytes>>,
    baseline: Option<Measurement<MemoryBytes>>,
    diff: Option<Result<HexDiff, String>>,
    filter_readable: bool,
}

impl MemoryPanel {
    pub(crate) fn new(regions: PlatformResult<Vec<VirtualMemoryRegion>>) -> Self {
        Self {
            regions,
            selected: None,
            sample: None,
            baseline: None,
            diff: None,
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
                                        let length = (region.size_bytes as usize)
                                            .clamp(1, MAX_MEMORY_SAMPLE_BYTES);
                                        let key = SampleKey {
                                            region_index: index,
                                            address: region.base_address,
                                            length,
                                        };
                                        self.selected = Some(key);
                                        self.baseline = None;
                                        self.diff = None;
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
            (Some(key), Some(sample)) => {
                ui.small(format!(
                    "Регион #{} · 0x{:016X} · {} байт",
                    key.region_index, key.address, key.length
                ));
                show_sample(ui, sample);
                self.snapshot_controls(ui, read_bytes);
            }
            _ => {
                ui.label("Выберите читаемый регион кнопкой HEX.");
            }
        }

        if let Some(diff) = &self.diff {
            ui.add_space(12.0);
            ui.heading("Сравнение снимков");
            match diff {
                Ok(diff) => show_diff(ui, diff),
                Err(reason) => {
                    ui.colored_label(ui.visuals().error_fg_color, reason);
                }
            }
        }
    }

    fn snapshot_controls(
        &mut self,
        ui: &mut egui::Ui,
        read_bytes: &dyn Fn(MemoryReadRequest) -> PlatformResult<MemoryBytes>,
    ) {
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            let can_pin = matches!(&self.sample, Some(PlatformResult::Success(_)));
            if ui
                .add_enabled(can_pin, egui::Button::new("Запомнить снимок A"))
                .clicked()
                && let Some(PlatformResult::Success(measurement)) = &self.sample
            {
                self.baseline = Some(measurement.clone());
                self.diff = None;
            }
            let can_compare = self.baseline.is_some() && self.selected.is_some();
            if ui
                .add_enabled(can_compare, egui::Button::new("Обновить и сравнить с A"))
                .clicked()
                && let Some(key) = self.selected.clone()
            {
                let refreshed = read_bytes(MemoryReadRequest {
                    address: key.address,
                    length: key.length,
                });
                self.diff = match (&self.baseline, &refreshed) {
                    (Some(before), PlatformResult::Success(after)) => {
                        Some(HexDiff::compare(before, after))
                    }
                    (_, other) => Some(Err(format!(
                        "Не удалось получить снимок B: {}",
                        other.availability()
                    ))),
                };
                self.sample = Some(refreshed);
            }
            if ui.button("Сбросить A").clicked() {
                self.baseline = None;
                self.diff = None;
            }
        });
        if let Some(baseline) = &self.baseline {
            ui.small(format!(
                "Снимок A: 0x{:016X} · {} байт · UTC {} · MEASURED",
                baseline.value.address,
                baseline.value.bytes.len(),
                baseline.timestamp.to_rfc3339()
            ));
        } else {
            ui.small(
                "Снимок A не задан. Сравнение считается CALCULATED по двум MEASURED выборкам.",
            );
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

fn show_diff(ui: &mut egui::Ui, diff: &HexDiff) {
    ui.monospace("CALCULATED");
    ui.label(format!(
        "Изменено {} из {} байт · A UTC {} → B UTC {}",
        diff.changed_offsets.len(),
        diff.before.len(),
        diff.before_at.to_rfc3339(),
        diff.after_at.to_rfc3339()
    ));
    if diff.changed_offsets.is_empty() {
        ui.label("Байтовых отличий нет.");
        return;
    }
    egui::ScrollArea::vertical()
        .id_salt("memory_hex_diff")
        .max_height(220.0)
        .show(ui, |ui| {
            egui::Grid::new("memory_diff_grid")
                .striped(true)
                .spacing([16.0, 4.0])
                .show(ui, |ui| {
                    ui.label("Смещение");
                    ui.label("A");
                    ui.label("B");
                    ui.label("ASCII");
                    ui.end_row();
                    for &offset in &diff.changed_offsets {
                        let before = diff.before[offset];
                        let after = diff.after[offset];
                        ui.monospace(format!("+{offset:04X}"));
                        ui.monospace(format!("{before:02X}"));
                        ui.monospace(format!("{after:02X}"));
                        ui.monospace(format!("{} → {}", ascii_dot(before), ascii_dot(after)));
                        ui.end_row();
                    }
                });
            ui.add_space(8.0);
            ui.small(
                "Подсветка только изменённых байт. Полный дамп B остаётся в HEX-просмотре выше.",
            );
        });
}

fn ascii_dot(byte: u8) -> char {
    if (0x20..0x7f).contains(&byte) {
        byte as char
    } else {
        '.'
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
            out.push(ascii_dot(*byte));
        }
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::UNIX_EPOCH;
    use u_monitor_core::{MemoryProtection, MemoryRegionKind, MemoryState};

    fn measured_bytes(address: u64, bytes: Vec<u8>) -> Measurement<MemoryBytes> {
        Measurement {
            value: MemoryBytes { address, bytes },
            source: DataSource::Measured,
            timestamp: UNIX_EPOCH.into(),
        }
    }

    #[test]
    fn hex_dump_keeps_address_ascii_and_padding() {
        let text = hex_dump(0x1000, &[0x41, 0x00, 0x7e, 0xff]);
        assert!(text.contains("0000000000001000"));
        assert!(text.contains("41 00 7E FF"));
        assert!(text.contains("A.~."));
    }

    #[test]
    fn compare_reports_only_changed_offsets_and_rejects_mismatched_ranges() {
        let before = measured_bytes(0x1000, vec![1, 2, 3, 4]);
        let after = measured_bytes(0x1000, vec![1, 9, 3, 8]);
        let diff = HexDiff::compare(&before, &after).unwrap();
        assert_eq!(diff.changed_offsets, vec![1, 3]);
        assert!(HexDiff::compare(&before, &measured_bytes(0x2000, vec![1, 2, 3, 4])).is_err());
        assert!(HexDiff::compare(&before, &measured_bytes(0x1000, vec![1, 2])).is_err());
    }

    #[test]
    fn identical_snapshots_produce_empty_change_list() {
        let sample = measured_bytes(0x40, vec![0xAA, 0xBB]);
        let diff = HexDiff::compare(&sample, &sample).unwrap();
        assert!(diff.changed_offsets.is_empty());
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
        panel.selected = Some(SampleKey {
            region_index: 0,
            address: 0x2000,
            length: 2,
        });
        panel.sample = Some(PlatformResult::Success(measured_bytes(
            0x2000,
            b"AB".to_vec(),
        )));
        panel.baseline = Some(measured_bytes(0x2000, b"AB".to_vec()));
        panel.diff = Some(Ok(HexDiff::compare(
            panel.baseline.as_ref().unwrap(),
            &measured_bytes(0x2000, b"AC".to_vec()),
        )
        .unwrap()));
        let output = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                panel.show(ui, &|_| PlatformResult::NotImplemented("unused".into()));
            });
        });
        assert!(!output.shapes.is_empty());
    }
}

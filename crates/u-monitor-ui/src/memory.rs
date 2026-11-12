//! Current-process virtual memory map and read-only HEX samples.
use crate::dashboard::{ascii_dot, hex_rows};
use eframe::egui;
use u_library_core::find_byte_offsets;
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
    address_input: String,
    search_input: String,
    length_input: usize,
    request_error: Option<&'static str>,
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
            address_input: String::new(),
            search_input: String::new(),
            length_input: MAX_MEMORY_SAMPLE_BYTES,
            request_error: None,
        }
    }

    pub(crate) fn show(
        &mut self,
        ui: &mut egui::Ui,
        read_bytes: &dyn Fn(MemoryReadRequest) -> PlatformResult<MemoryBytes>,
    ) {
        let mut reveal_sample = false;
        ui.horizontal_wrapped(|ui| {
            ui.label("Адрес");
            let address = ui.add(
                egui::TextEdit::singleline(&mut self.address_input)
                    .hint_text("0x…")
                    .desired_width(170.0),
            );
            ui.label("Байты");
            ui.add(egui::DragValue::new(&mut self.length_input).range(1..=MAX_MEMORY_SAMPLE_BYTES));
            if ui.button("Прочитать").clicked()
                || (address.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter)))
            {
                match self.requested_range() {
                    Ok(key) => {
                        self.read_selection(key, read_bytes);
                        reveal_sample = true;
                    }
                    Err(error) => {
                        self.request_error = Some(error);
                        self.selected = None;
                        self.sample = None;
                        self.baseline = None;
                        self.diff = None;
                    }
                }
            }
        });
        if let Some(error) = self.request_error {
            ui.colored_label(ui.visuals().error_fg_color, error);
        }
        ui.label("Карта виртуальной памяти выбранного процесса.");
        ui.add_space(8.0);
        ui.checkbox(
            &mut self.filter_readable,
            "Только читаемые COMMITTED-регионы",
        );
        ui.add_space(8.0);
        ui.monospace(self.regions.availability().to_string());
        match &self.regions {
            PlatformResult::Success(measurement) => {
                crate::design::source(ui, measurement.source);
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
                                        let length =
                                            region.size_bytes.min(MAX_MEMORY_SAMPLE_BYTES as u64)
                                                as usize;
                                        let key = SampleKey {
                                            region_index: index,
                                            address: region.base_address,
                                            length,
                                        };
                                        self.selected = Some(key);
                                        self.address_input =
                                            format!("0x{:016X}", region.base_address);
                                        self.length_input = length;
                                        self.request_error = None;
                                        self.baseline = None;
                                        self.diff = None;
                                        self.sample = Some(read_bytes(MemoryReadRequest {
                                            address: region.base_address,
                                            length,
                                        }));
                                        reveal_sample = true;
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
        let heading = ui.heading("HEX-просмотр");
        if reveal_sample {
            heading.scroll_to_me(Some(egui::Align::TOP));
        }
        ui.label(format!(
            "Только чтение, максимум {MAX_MEMORY_SAMPLE_BYTES} байт за запрос."
        ));
        match (&self.selected, &self.sample) {
            (Some(key), Some(sample)) => {
                ui.small(format!(
                    "Регион #{} · 0x{:016X} · {} байт",
                    key.region_index, key.address, key.length
                ));
                show_sample(ui, sample, reveal_sample, &mut self.search_input);
                self.snapshot_controls(ui, read_bytes);
            }
            _ => {
                ui.label("Выберите читаемый регион кнопкой HEX.");
            }
        }

        if let Some(diff) = &self.diff {
            ui.add_space(12.0);
            crate::design::section(ui, "Сравнение снимков");
            match diff {
                Ok(diff) => show_diff(ui, diff),
                Err(reason) => {
                    ui.colored_label(ui.visuals().error_fg_color, reason);
                }
            }
        }
    }

    fn requested_range(&self) -> Result<SampleKey, &'static str> {
        let address = crate::simulation::parse_address(&self.address_input)
            .ok_or("Введите адрес в десятичном виде или с префиксом 0x.")?;
        let length = self.length_input;
        if !(1..=MAX_MEMORY_SAMPLE_BYTES).contains(&length) {
            return Err("Недопустимое число байт для чтения.");
        }
        let end = address
            .checked_add(length as u64)
            .ok_or("Диапазон выходит за пределы адресного пространства.")?;
        let PlatformResult::Success(regions) = &self.regions else {
            return Err("Карта памяти недоступна.");
        };
        let region_index = regions
            .value
            .iter()
            .position(|region| {
                region.readable
                    && address >= region.base_address
                    && end <= region.base_address.saturating_add(region.size_bytes)
            })
            .ok_or("Диапазон должен целиком находиться в одном читаемом регионе.")?;
        Ok(SampleKey {
            region_index,
            address,
            length,
        })
    }

    fn read_selection(
        &mut self,
        key: SampleKey,
        read_bytes: &dyn Fn(MemoryReadRequest) -> PlatformResult<MemoryBytes>,
    ) {
        let sample = read_bytes(MemoryReadRequest {
            address: key.address,
            length: key.length,
        });
        self.selected = Some(key);
        self.sample = Some(sample);
        self.request_error = None;
        self.baseline = None;
        self.diff = None;
    }

    fn snapshot_controls(
        &mut self,
        ui: &mut egui::Ui,
        read_bytes: &dyn Fn(MemoryReadRequest) -> PlatformResult<MemoryBytes>,
    ) {
        ui.add_space(8.0);
        ui.horizontal_wrapped(|ui| {
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

fn show_sample(
    ui: &mut egui::Ui,
    sample: &PlatformResult<MemoryBytes>,
    reset_scroll: bool,
    search_input: &mut String,
) {
    ui.monospace(sample.availability().to_string());
    match sample {
        PlatformResult::Success(measurement) => {
            crate::design::source(ui, measurement.source);
            ui.small(format!(
                "0x{:016X} · {} байт · UTC {}",
                measurement.value.address,
                measurement.value.bytes.len(),
                measurement.timestamp.to_rfc3339()
            ));
            show_search(ui, search_input, &measurement.value.bytes);
            ui.small("Смещение от начала снимка · HEX · ASCII");
            hex_rows(
                ui,
                "memory_hex",
                None,
                &measurement.value.bytes,
                reset_scroll,
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

fn parse_hex_pattern(input: &str) -> Result<Vec<u8>, &'static str> {
    let digits: Vec<_> = input
        .bytes()
        .filter(|byte| !byte.is_ascii_whitespace())
        .take(2 * MAX_MEMORY_SAMPLE_BYTES + 1)
        .collect();
    if digits.len() > 2 * MAX_MEMORY_SAMPLE_BYTES {
        return Err("Шаблон длиннее максимального снимка (512 байт).");
    }
    if !digits.len().is_multiple_of(2) || !digits.iter().all(u8::is_ascii_hexdigit) {
        return Err("Введите по две HEX-цифры на байт, например 00 FF.");
    }
    digits
        .chunks_exact(2)
        .map(|pair| {
            let pair = std::str::from_utf8(pair).map_err(|_| "Недопустимые HEX-цифры.")?;
            u8::from_str_radix(pair, 16).map_err(|_| "Недопустимые HEX-цифры.")
        })
        .collect()
}

fn show_search(ui: &mut egui::Ui, input: &mut String, bytes: &[u8]) {
    ui.horizontal_wrapped(|ui| {
        ui.label("Поиск HEX в снимке");
        ui.add(
            egui::TextEdit::singleline(input)
                .hint_text("00 FF")
                .desired_width(170.0),
        );
    });
    match parse_hex_pattern(input) {
        Ok(pattern) if pattern.is_empty() => {}
        Ok(pattern) => {
            let offsets: Vec<_> = find_byte_offsets(bytes, &pattern).collect();
            if offsets.is_empty() {
                ui.small("Совпадений нет.");
                return;
            }
            ui.small(format!("Совпадений: {} · смещения", offsets.len()));
            egui::ScrollArea::vertical()
                .id_salt("memory_hex_matches")
                .max_height(72.0)
                .show(ui, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        for offset in offsets {
                            ui.monospace(format!("+{offset:04X}"));
                        }
                    });
                });
        }
        Err(message) => {
            ui.colored_label(ui.visuals().error_fg_color, message);
        }
    }
}

fn show_diff(ui: &mut egui::Ui, diff: &HexDiff) {
    crate::design::source(ui, DataSource::Calculated);
    ui.label(format!(
        "Изменено {} из {} байт · A UTC {} · B UTC {}",
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
        .min_scrolled_height(160.0)
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
            ui.small("В таблице показаны изменённые байты. Полный дамп B доступен выше.");
        });
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
    fn hex_row_keeps_ascii_and_padding() {
        let text = crate::dashboard::hex_data(&[0x41, 0x00, 0x7e, 0xff]);
        assert_eq!(text, format!("{:23}  A.~.", "41 00 7E FF"));
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
        panel.address_input = "0x2FFE".into();
        panel.length_input = 2;
        let request = panel
            .requested_range()
            .expect("read ends at region boundary");
        assert_eq!(
            (request.region_index, request.address, request.length),
            (0, 0x2FFE, 2)
        );
        panel.length_input = 3;
        assert!(panel.requested_range().is_err());
        panel.address_input = u64::MAX.to_string();
        assert!(panel.requested_range().is_err());
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
    #[test]
    fn hex_search_accepts_compact_and_spaced_bytes() {
        assert_eq!(parse_hex_pattern("00ffA1"), Ok(vec![0, 255, 161]));
        assert_eq!(parse_hex_pattern("00 ff\tA1"), Ok(vec![0, 255, 161]));
        assert_eq!(parse_hex_pattern("  "), Ok(Vec::new()));
    }

    #[test]
    fn hex_search_rejects_invalid_and_oversized_patterns() {
        for input in ["f", "0x", "+F", "я"] {
            assert!(parse_hex_pattern(input).is_err(), "{input}");
        }
        assert!(parse_hex_pattern(&"00".repeat(MAX_MEMORY_SAMPLE_BYTES + 1)).is_err());
    }
}

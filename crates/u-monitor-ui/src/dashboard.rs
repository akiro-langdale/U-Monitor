use crate::live::LiveTelemetry;
use crate::{app::Page, design};
use eframe::egui::{self, Color32, RichText};
use u_monitor_core::{CacheTopology, DataSource, PlatformResult, RegisterSnapshot, StorageVolume};

const CYAN: Color32 = Color32::from_rgb(86, 205, 234);
const MINT: Color32 = Color32::from_rgb(95, 210, 180);

fn accent(ui: &egui::Ui, color: Color32) -> Color32 {
    if ui.ctx().theme() == egui::Theme::Dark {
        color
    } else {
        ui.visuals().hyperlink_color
    }
}

fn value<T>(result: &PlatformResult<T>) -> Option<&T> {
    match result {
        PlatformResult::Success(measurement) => Some(&measurement.value),
        _ => None,
    }
}

fn gib(bytes: &u64) -> String {
    format!("{:.1} ГиБ", *bytes as f64 / 1_073_741_824.0)
}

pub(crate) fn show(
    ui: &mut egui::Ui,
    live: &LiveTelemetry,
    cache: &PlatformResult<CacheTopology>,
    storage: &PlatformResult<Vec<StorageVolume>>,
    registers: &PlatformResult<RegisterSnapshot>,
) -> Option<Page> {
    let mut next = None;
    let snapshot = live.latest();
    let cpu = snapshot.and_then(|s| value(&s.cpu));
    let ram = snapshot.and_then(|s| value(&s.ram));
    let levels = value(cache).and_then(|topology| {
        let mut levels: Vec<_> = topology.nodes.iter().map(|node| node.level).collect();
        levels.sort_unstable();
        levels.dedup();
        (!levels.is_empty()).then(|| {
            levels
                .iter()
                .map(|level| format!("L{level}"))
                .collect::<Vec<_>>()
                .join(" · ")
        })
    });
    let metrics = [
        (
            "CPU",
            cpu.and_then(|info| value(&info.total_usage))
                .map(|usage| format!("{:.1}%", usage.get())),
            Page::Cpu,
        ),
        (
            "RAM · используется",
            ram.and_then(|info| value(&info.used_bytes)).map(gib),
            Page::Ram,
        ),
        ("Кэш CPU", levels, Page::Cache),
        (
            "Подключённые тома",
            value(storage).map(|volumes| volumes.len().to_string()),
            Page::Storage,
        ),
    ];
    ui.horizontal_wrapped(|ui| {
        if live.is_live() {
            ui.colored_label(accent(ui, MINT), "LIVE");
        }
        if let Some(snapshot) = snapshot {
            ui.small(format!(
                "Телеметрия · UTC {}",
                snapshot.captured_at.to_rfc3339()
            ));
        } else {
            ui.small("Снимок телеметрии пока недоступен");
        }
    });
    let count = if ui.available_width() >= 780.0 { 4 } else { 2 };
    for row in metrics.chunks(count) {
        ui.columns(count, |columns| {
            for (column, (label, value, page)) in columns.iter_mut().zip(row) {
                design::surface(column).show(column, |ui| {
                    ui.set_min_width(ui.available_width());
                    if ui.link(*label).clicked() {
                        next = Some(*page);
                    }
                    ui.label(
                        RichText::new(value.as_deref().unwrap_or("Нет данных"))
                            .size(22.0)
                            .monospace()
                            .color(ui.visuals().hyperlink_color),
                    );
                });
            }
        });
        ui.add_space(8.0);
    }

    design::surface(ui).show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        if ui.link("Регистры CPU").clicked() {
            next = Some(Page::Registers);
        }
        if let PlatformResult::Success(snapshot) = registers {
            ui.horizontal_wrapped(|ui| {
                design::source(ui, snapshot.source);
                ui.small(format!("UTC {}", snapshot.timestamp.to_rfc3339()));
            });
            for row in [["RAX", "RBX"], ["RCX", "RIP"]] {
                ui.columns(2, |columns| {
                    for (column, name) in columns.iter_mut().zip(row) {
                        let register = snapshot.value.registers.iter().find(|r| r.name == name);
                        if let Some(register) = register {
                            column.monospace(format!("{name}  0x{:X}", register.masked_bits()));
                        } else {
                            column.monospace(format!("{name}  —"));
                        }
                    }
                });
            }
        } else {
            ui.small(registers.availability().to_string());
        }
    });
    ui.add_space(7.0);
    design::surface(ui).show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.horizontal_wrapped(|ui| {
            if ui.link("Использование RAM").clicked() {
                next = Some(Page::Ram);
            }
        });
        if let Some((info, fraction)) =
            ram.and_then(|info| crate::ram::usage(info).map(|fraction| (info, fraction)))
        {
            ui.horizontal_wrapped(|ui| {
                ui.strong(format!("{:.1}%", fraction * 100.0));
                design::source(ui, DataSource::Calculated);
            });
            ui.add(
                egui::ProgressBar::new(fraction)
                    .fill(accent(ui, MINT))
                    .desired_height(9.0),
            );
            if let Some(total) = value(&info.total_bytes) {
                ui.small(format!("Всего видно ОС · {}", gib(total)));
            }
        } else {
            ui.small("Данные RAM недоступны");
        }
    });
    next
}

pub(crate) fn ascii_dot(byte: u8) -> char {
    if (0x20..0x7f).contains(&byte) {
        char::from(byte)
    } else {
        '.'
    }
}

pub(crate) fn hex_rows(
    ui: &mut egui::Ui,
    id: &str,
    file_offset: Option<u64>,
    bytes: &[u8],
    reset_scroll: bool,
) {
    if bytes.is_empty() {
        ui.label("Прочитано 0 байт");
        return;
    }
    let mut scroll_area = egui::ScrollArea::both()
        .id_salt(id)
        .min_scrolled_height(224.0)
        .max_height(280.0);
    if reset_scroll {
        scroll_area = scroll_area.vertical_scroll_offset(0.0);
    }
    scroll_area.show(ui, |ui| {
        egui::Grid::new(id)
            .striped(true)
            .spacing([20.0, 4.0])
            .min_row_height(21.0)
            .show(ui, |ui| {
                for header in ["Смещение", "00 01 02 03 04 05 06 07", "ASCII"] {
                    ui.label(RichText::new(header).monospace().weak());
                }
                ui.end_row();
                for (row, bytes) in bytes.chunks(8).enumerate() {
                    let offset = (row * 8) as u64;
                    let address = match file_offset {
                        Some(base) => format!("{:016X}", base.saturating_add(offset)),
                        None => format!("{offset:04X}"),
                    };
                    let data = hex_data(bytes);
                    let (hex, ascii) = data.split_at(25);
                    ui.label(RichText::new(address).monospace().color(accent(ui, CYAN)));
                    ui.monospace(hex.trim_end());
                    ui.label(RichText::new(ascii).monospace().color(accent(ui, MINT)));
                    ui.end_row();
                }
            });
    });
}

pub(crate) fn hex_data(bytes: &[u8]) -> String {
    let hex = bytes
        .iter()
        .map(|byte| format!("{byte:02X}"))
        .collect::<Vec<_>>()
        .join(" ");
    let ascii: String = bytes.iter().copied().map(ascii_dot).collect();
    format!("{hex:23}  {ascii}")
}

//! Displays the startup volume snapshot without filesystem access during frames.
use eframe::egui;
use u_monitor_core::{DataSource, PlatformResult, StorageVolume};

pub(crate) fn show(ui: &mut egui::Ui, result: &PlatformResult<Vec<StorageVolume>>) {
    ui.label("Снимок смонтированных томов при запуске приложения.");
    ui.monospace(result.availability().to_string());
    let snapshot = match result {
        PlatformResult::Success(snapshot) => snapshot,
        _ => {
            failure(ui, result);
            return;
        }
    };
    ui.monospace(source(snapshot.source));
    ui.small(format!("UTC {}", snapshot.timestamp.to_rfc3339()));
    ui.label("Том — смонтированная файловая система. Несколько томов могут использовать один физический накопитель; их объёмы не следует суммировать для определения физической ёмкости.");
    ui.label("Занято = общий объём − доступно (CALCULATED); показатель может учитывать резервы и квоты. 1 GiB = 1 073 741 824 байта.");
    if snapshot.value.is_empty() {
        ui.label("Смонтированные тома не обнаружены.");
        return;
    }
    ui.label(format!(
        "Томов: {} · таблица прокручивается по горизонтали",
        snapshot.value.len()
    ));
    egui::ScrollArea::horizontal()
        .id_salt("storage_horizontal")
        .show(ui, |ui| {
            egui::Grid::new("storage_volumes")
                .striped(true)
                .spacing([16.0, 12.0])
                .show(ui, |ui| {
                    for heading in [
                        "Имя тома",
                        "Точка монтирования",
                        "Файловая система",
                        "Тип по данным ОС",
                        "Общий объём",
                        "Доступно",
                        "Занято",
                    ] {
                        ui.strong(heading);
                    }
                    ui.end_row();
                    for volume in &snapshot.value {
                        field(ui, &volume.name, |name| name.to_string_lossy().into_owned());
                        ui.vertical(|ui| {
                            ui.set_width(170.0);
                            ui.label(volume.mount_point.to_string_lossy());
                        });
                        field(ui, &volume.file_system, |fs| {
                            fs.to_string_lossy().into_owned()
                        });
                        ui.vertical(|ui| {
                            ui.set_width(145.0);
                            ui.monospace(volume.kind.to_string());
                        });
                        field(ui, &volume.space.total_bytes, bytes);
                        field(ui, &volume.space.available_bytes, bytes);
                        field(ui, &volume.space.used_bytes, bytes);
                        ui.end_row();
                    }
                });
        });
}

fn bytes(value: &u64) -> String {
    format!("{value} байт · {:.2} GiB", *value as f64 / 1_073_741_824.0)
}

fn source(value: DataSource) -> &'static str {
    match value {
        DataSource::Measured => "MEASURED",
        DataSource::Calculated => "CALCULATED",
        DataSource::Simulated => "SIMULATED",
    }
}

fn failure<T>(ui: &mut egui::Ui, result: &PlatformResult<T>) {
    match result {
        PlatformResult::PermissionRequired(reason)
        | PlatformResult::Unavailable(reason)
        | PlatformResult::NotImplemented(reason) => {
            ui.label(reason);
        }
        PlatformResult::Error(error) => {
            ui.colored_label(ui.visuals().error_fg_color, error.to_string());
        }
        PlatformResult::Success(_) => {}
    }
}

fn field<T>(ui: &mut egui::Ui, result: &PlatformResult<T>, value: impl FnOnce(&T) -> String) {
    ui.vertical(|ui| {
        ui.set_width(190.0);
        ui.monospace(result.availability().to_string());
        match result {
            PlatformResult::Success(measurement) => {
                ui.label(value(&measurement.value));
                ui.monospace(source(measurement.source));
                ui.small(format!("UTC {}", measurement.timestamp.to_rfc3339()));
            }
            _ => failure(ui, result),
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{ffi::OsString, path::PathBuf, time::UNIX_EPOCH};
    use u_monitor_core::{DiagnosticError, Measurement, StorageKind, VolumeSpace};

    fn measured<T>(value: T) -> PlatformResult<T> {
        PlatformResult::Success(Measurement {
            value,
            source: DataSource::Measured,
            timestamp: UNIX_EPOCH.into(),
        })
    }

    fn rendered_text(result: &PlatformResult<Vec<StorageVolume>>, dark: bool) -> String {
        let ctx = egui::Context::default();
        ctx.set_theme(if dark {
            egui::Theme::Dark
        } else {
            egui::Theme::Light
        });
        let mut text = String::new();
        for _ in 0..2 {
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(2000.0, 1200.0),
                )),
                ..Default::default()
            };
            let output = ctx.run(input, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| show(ui, result));
            });
            fn collect(shape: &egui::Shape, text: &mut String) {
                match shape {
                    egui::Shape::Text(shape) => {
                        text.push_str(&shape.galley.job.text);
                        text.push('\n');
                    }
                    egui::Shape::Vec(shapes) => {
                        for shape in shapes {
                            collect(shape, text);
                        }
                    }
                    _ => {}
                }
            }
            text.clear();
            for shape in output.shapes {
                collect(&shape.shape, &mut text);
            }
        }
        text
    }

    #[test]
    fn empty_enumeration_and_top_level_failures_are_distinct() {
        let text = rendered_text(&measured(vec![]), true);
        assert!(text.contains("Смонтированные тома не обнаружены"));
        assert!(text.contains("SUPPORTED"));
        for result in [
            PlatformResult::PermissionRequired("volume permission denied".into()),
            PlatformResult::Unavailable("volumes unavailable".into()),
            PlatformResult::NotImplemented("volumes pending".into()),
            PlatformResult::Error(DiagnosticError::PlatformError("enumeration failed".into())),
        ] {
            let text = rendered_text(&result, true);
            assert!(text.contains(&result.availability().to_string()));
            assert!(!text.contains("Смонтированные тома не обнаружены"));
            assert!(!text.contains("MEASURED"));
        }
    }

    #[test]
    fn partial_volume_keeps_unicode_zero_space_unknown_kind_and_field_errors() {
        let result = measured(vec![StorageVolume {
            name: measured(OsString::from("Архив 数据")),
            mount_point: PathBuf::from("/mnt/Архив"),
            file_system: PlatformResult::Unavailable("filesystem missing".into()),
            kind: StorageKind::Unknown(42),
            space: VolumeSpace {
                total_bytes: measured(1_073_741_824),
                available_bytes: measured(0),
                used_bytes: PlatformResult::Error(DiagnosticError::PlatformError(
                    "space inconsistent".into(),
                )),
            },
        }]);
        for dark in [false, true] {
            let text = rendered_text(&result, dark);
            for expected in [
                "Архив 数据",
                "/mnt/Архив",
                "UNKNOWN (42)",
                "filesystem missing",
                "space inconsistent",
                "0 байт · 0.00 GiB",
                "1073741824 байт · 1.00 GiB",
                "1970-01-01T00:00:00+00:00",
            ] {
                assert!(text.contains(expected), "missing {expected}: {text}");
            }
        }
    }

    #[test]
    fn calculated_used_space_retains_provenance_alongside_measured_values() {
        let result = measured(vec![StorageVolume {
            name: measured(OsString::from("Data")),
            mount_point: PathBuf::from("/data"),
            file_system: measured(OsString::from("testfs")),
            kind: StorageKind::Ssd,
            space: VolumeSpace {
                total_bytes: measured(2048),
                available_bytes: measured(1024),
                used_bytes: PlatformResult::Success(Measurement {
                    value: 1024,
                    source: DataSource::Calculated,
                    timestamp: UNIX_EPOCH.into(),
                }),
            },
        }]);
        let text = rendered_text(&result, true);
        assert!(text.contains("SSD"));
        assert!(text.contains("1024 байт"));
        // The row label is separate from the explanation above the table.
        assert!(text.lines().any(|line| line == "CALCULATED"));
        assert!(text.lines().any(|line| line == "MEASURED"));
    }
}

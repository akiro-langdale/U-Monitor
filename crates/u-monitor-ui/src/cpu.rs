//! CPU discovery at startup plus live utilization from shared telemetry.
use crate::live::LiveTelemetry;
use eframe::egui;
use u_monitor_core::{CpuInfo, PlatformResult};

pub(crate) mod history;

pub(crate) fn show(
    ui: &mut egui::Ui,
    result: &PlatformResult<CpuInfo>,
    live: &mut LiveTelemetry,
    selected_logical: Option<usize>,
) -> Option<usize> {
    ui.label("Сведения CPU получены при запуске приложения.");
    let field_width = (ui.available_width() - 190.0).clamp(140.0, 420.0);
    field(ui, result, field_width, |_, _| {});
    if let PlatformResult::Success(snapshot) = result {
        egui::Grid::new("cpu_discovery")
            .striped(true)
            .spacing([20.0, 12.0])
            .show(ui, |ui| {
                ui.label("Модели процессора");
                field(
                    ui,
                    &snapshot.value.model_names,
                    field_width,
                    |ui, models| {
                        for model in models {
                            ui.label(model);
                        }
                    },
                );
                ui.end_row();
                ui.label("Физические ядра");
                field(
                    ui,
                    &snapshot.value.physical_cores,
                    field_width,
                    |ui, count| {
                        ui.label(count.to_string());
                    },
                );
                ui.end_row();
                ui.label("Логические процессоры");
                field(
                    ui,
                    &snapshot.value.logical_processors,
                    field_width,
                    |ui, count| {
                        ui.label(count.to_string());
                    },
                );
                ui.end_row();
            });
    }
    ui.add_space(12.0);
    ui.label("Физическое ядро — вычислительное ядро CPU. Логический процессор — аппаратный поток, доступный планировщику ОС. Их количество может различаться.");
    ui.label("Виртуальная машина или контейнер могут ограничивать видимую топологию. Данные относятся к представлению ОС на момент запуска.");
    ui.add_space(16.0);
    crate::design::section(ui, "Живая нагрузка");
    ui.label(egui::RichText::new("Источник: фоновый TelemetryWorker · MEASURED").weak());
    ui.label("Клик по строке логического CPU выбирает его для Cache/Simulation.");
    ui.add_space(8.0);
    live.show_cpu_live(ui, selected_logical)
}

fn field<T>(
    ui: &mut egui::Ui,
    result: &PlatformResult<T>,
    width: f32,
    value: impl FnOnce(&mut egui::Ui, &T),
) {
    ui.vertical(|ui| {
        ui.set_width(width);
        ui.monospace(result.availability().to_string());
        match result {
            PlatformResult::Success(measurement) => {
                value(ui, &measurement.value);
                crate::design::source(ui, measurement.source);
                ui.small(format!("UTC {}", measurement.timestamp.to_rfc3339()));
            }
            PlatformResult::PermissionRequired(reason)
            | PlatformResult::Unavailable(reason)
            | PlatformResult::NotImplemented(reason) => {
                ui.label(reason);
            }
            PlatformResult::Error(error) => {
                ui.colored_label(ui.visuals().error_fg_color, error.to_string());
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{num::NonZeroUsize, time::UNIX_EPOCH};
    use u_monitor_core::{DiagnosticError, Measurement};

    fn measured<T>(value: T) -> PlatformResult<T> {
        PlatformResult::Success(Measurement {
            value,
            source: u_monitor_core::DataSource::Measured,
            timestamp: UNIX_EPOCH.into(),
        })
    }

    fn rendered_text(result: &PlatformResult<CpuInfo>, dark: bool) -> String {
        let ctx = egui::Context::default();
        ctx.set_theme(if dark {
            egui::Theme::Dark
        } else {
            egui::Theme::Light
        });
        let mut live = LiveTelemetry::new(Err(DiagnosticError::PlatformError("test".into())));
        let mut text = String::new();
        for _ in 0..2 {
            let output = ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    show(ui, result, &mut live, None);
                });
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
    fn partial_topology_preserves_models_counts_and_per_field_failures() {
        let result = measured(CpuInfo {
            model_names: measured(vec!["Model A".into(), "Model B".into()]),
            physical_cores: PlatformResult::Unavailable("physical topology missing".into()),
            logical_processors: measured(NonZeroUsize::new(16).unwrap()),
        });
        for dark in [false, true] {
            let text = rendered_text(&result, dark);
            for expected in [
                "Model A",
                "Model B",
                "16",
                "UNAVAILABLE",
                "physical topology missing",
                "MEASURED",
                "1970-01-01T00:00:00+00:00",
                "Живая нагрузка",
            ] {
                assert!(text.contains(expected), "missing {expected}: {text}");
            }
        }
    }

    #[test]
    fn top_level_failures_do_not_show_fabricated_topology() {
        for result in [
            PlatformResult::PermissionRequired("permission missing".into()),
            PlatformResult::Unavailable("CPU missing".into()),
            PlatformResult::NotImplemented("CPU pending".into()),
            PlatformResult::Error(DiagnosticError::PlatformError("CPU read failed".into())),
        ] {
            let text = rendered_text(&result, true);
            assert!(text.contains(&result.availability().to_string()));
            assert!(text.contains("Живая нагрузка"));
            assert!(!text.contains("Физические ядра"));
            assert!(!text.contains("Модели процессора"));
        }
    }
}

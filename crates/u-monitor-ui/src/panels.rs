use crate::app::Page;
use eframe::egui;
use u_monitor_core::{Availability, DataSource, PlatformResult, SystemIdentity};

pub(crate) fn overview(ui: &mut egui::Ui, identity: &PlatformResult<SystemIdentity>) {
    egui::Frame::group(ui.style()).show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.heading("Среда выполнения");
        ui.add_space(8.0);
        ui.horizontal_wrapped(|ui| {
            ui.label("Получение сведений:");
            ui.monospace(identity.availability().to_string());
        });
        match identity {
            PlatformResult::Success(measurement) => {
                egui::Grid::new("system_identity").spacing([24.0, 10.0]).show(ui, |ui| {
                    ui.label("ОС целевой сборки");
                    ui.label(measurement.value.operating_system.to_string());
                    ui.end_row();
                    ui.label("Архитектура процесса");
                    ui.label(measurement.value.process_architecture.to_string());
                    ui.end_row();
                    ui.label("Источник");
                    ui.monospace(match measurement.source {
                        DataSource::Measured => "MEASURED",
                        DataSource::Calculated => "CALCULATED",
                        DataSource::Simulated => "SIMULATED",
                    });
                    ui.end_row();
                    ui.label("Время получения (UTC)");
                    ui.label(measurement.timestamp.to_rfc3339());
                    ui.end_row();
                });
                ui.add_space(10.0);
                ui.label("Сведения относятся к целевой платформе бинарного файла. Версия ОС и физическая архитектура CPU при эмуляции здесь не определяются.");
            }
            PlatformResult::PermissionRequired(reason)
            | PlatformResult::Unavailable(reason)
            | PlatformResult::NotImplemented(reason) => { ui.label(reason); }
            PlatformResult::Error(error) => { ui.label(error.to_string()); }
        }
    });
    ui.add_space(24.0);
    ui.heading("Разделы приложения");
    ui.label("Статусы ниже относятся к подключению панелей GUI.");
    ui.add_space(12.0);
    for page in Page::ALL.into_iter().filter(|page| *page != Page::Overview) {
        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.strong(page.title());
                ui.monospace(if matches!(page, Page::Ram | Page::Cpu | Page::Storage) {
                    "ПАНЕЛЬ ПОДКЛЮЧЕНА"
                } else {
                    "NOT_IMPLEMENTED"
                });
            });
            ui.label(match page {
                Page::Cpu => "Модели CPU, физические ядра и логические процессоры по данным ОС.",
                Page::Storage => {
                    "Смонтированные тома, файловые системы и доступное пространство по данным ОС."
                }
                Page::Ram => {
                    "Реальные снимки RAM, управление интервалом и график истории использования."
                }
                _ => page.pending(),
            });
        });
        ui.add_space(6.0);
    }
}

pub(crate) fn pending(ui: &mut egui::Ui, page: Page) {
    egui::Frame::group(ui.style()).show(ui, |ui| {
        ui.monospace(Availability::NotImplemented.to_string());
        ui.add_space(12.0);
        ui.label(page.pending());
    });
}

use eframe::egui;
use u_monitor_core::{Availability, PlatformFeature, PlatformResult, SystemIdentity};

pub(crate) fn overview(
    ui: &mut egui::Ui,
    identity: &PlatformResult<SystemIdentity>,
    capabilities: &[(PlatformFeature, Availability)],
) {
    crate::design::surface(ui).show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        crate::design::section(ui, "Среда выполнения");
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
                    crate::design::source(ui, measurement.source);
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
    crate::design::section(ui, "Возможности платформы");
    ui.label(
        "Статусы относятся к NativePlatformAdapter на текущем OS/arch. NOT_IMPLEMENTED — нет backend, а не ошибка измерения.",
    );
    ui.add_space(8.0);
    egui::Grid::new("platform_capabilities")
        .striped(true)
        .spacing([24.0, 4.0])
        .show(ui, |ui| {
            ui.label("Функция");
            ui.label("Статус");
            ui.end_row();
            for (feature, availability) in capabilities {
                ui.monospace(feature.to_string());
                ui.monospace(availability.to_string());
                ui.end_row();
            }
        });
}

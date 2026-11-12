use crate::design;
use eframe::egui;
use u_library_gpu::{list_devices, GpuDevice, GpuError};

pub(crate) struct GpuPanel {
    devices: Result<Vec<GpuDevice>, String>,
}

impl GpuPanel {
    pub(crate) fn load() -> Self {
        Self {
            devices: list_devices().map_err(format_error),
        }
    }

    pub(crate) fn show(&mut self, ui: &mut egui::Ui) {
        design::section(ui, "GPU adapters");
        ui.weak(
            "Список адаптеров через U-Library (DXGI на Windows). Содержимое VRAM не читается.",
        );
        ui.add_space(8.0);
        if ui.button("Обновить").clicked() {
            self.devices = list_devices().map_err(format_error);
        }
        ui.add_space(8.0);
        match &self.devices {
            Ok(devices) => {
                egui::Grid::new("gpu-adapters")
                    .striped(true)
                    .num_columns(5)
                    .show(ui, |ui| {
                        ui.strong("Имя");
                        ui.strong("Vendor");
                        ui.strong("Device");
                        ui.strong("Dedicated");
                        ui.strong("Shared");
                        ui.end_row();
                        for device in devices {
                            ui.label(&device.name);
                            ui.monospace(format!("0x{:04X}", device.vendor_id));
                            ui.monospace(format!("0x{:04X}", device.device_id));
                            ui.monospace(format_bytes(device.dedicated_bytes));
                            ui.monospace(format_bytes(device.shared_bytes));
                            ui.end_row();
                        }
                    });
            }
            Err(error) => {
                ui.colored_label(ui.visuals().warn_fg_color, error);
            }
        }
    }
}

fn format_bytes(value: Option<u64>) -> String {
    match value {
        Some(bytes) => format!("{:.1} ГиБ", bytes as f64 / 1_073_741_824.0),
        None => "UNAVAILABLE".into(),
    }
}

fn format_error(error: GpuError) -> String {
    error.to_string()
}

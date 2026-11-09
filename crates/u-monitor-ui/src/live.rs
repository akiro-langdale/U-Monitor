//! Shared background telemetry for RAM and CPU panels.
use crate::{cpu::history::CpuHistory, ram::history::RamHistory};
use eframe::egui;
use std::{sync::mpsc::TryRecvError, time::Duration};
use u_monitor_core::{
    CpuUsagePercent, DataSource, DiagnosticError, LogicalCpuUsage, PlatformResult,
};
use u_monitor_telemetry::{TelemetrySnapshot, TelemetryWorker};

pub(crate) struct LiveTelemetry {
    worker: Option<TelemetryWorker>,
    latest: Option<TelemetrySnapshot>,
    ram_history: RamHistory,
    cpu_history: CpuHistory,
    error: Option<DiagnosticError>,
    disconnected: bool,
    interval_ms: u64,
    dropped: u64,
}

impl LiveTelemetry {
    pub(crate) fn new(worker: Result<TelemetryWorker, DiagnosticError>) -> Self {
        let (worker, error) = match worker {
            Ok(w) => (Some(w), None),
            Err(e) => (None, Some(e)),
        };
        Self {
            worker,
            error,
            latest: None,
            ram_history: RamHistory::default(),
            cpu_history: CpuHistory::default(),
            disconnected: false,
            interval_ms: 1000,
            dropped: 0,
        }
    }

    pub(crate) fn poll(&mut self, ctx: &egui::Context) {
        if let Some(worker) = self.worker.as_ref().filter(|_| !self.disconnected) {
            for _ in 0..8 {
                match worker.try_recv() {
                    Ok(snapshot) => {
                        self.ram_history.push(&snapshot);
                        self.cpu_history.push(&snapshot);
                        self.latest = Some(snapshot);
                    }
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => {
                        self.disconnected = true;
                        self.error = Some(DiagnosticError::PlatformError(
                            "Поток телеметрии завершён. Ниже — последний полученный снимок.".into(),
                        ));
                        break;
                    }
                }
            }
            self.dropped = worker.dropped_notifications();
            ctx.request_repaint_after(Duration::from_millis(100));
        }
    }

    pub(crate) fn shutdown(&mut self) -> Result<(), DiagnosticError> {
        match self.worker.take() {
            Some(w) => w.stop()?.result,
            None => Ok(()),
        }
    }

    fn controls(&mut self, ui: &mut egui::Ui) {
        let before = self.interval_ms;
        ui.add_enabled_ui(self.worker.is_some() && !self.disconnected, |ui| {
            egui::ComboBox::from_id_salt("live_interval")
                .selected_text(format!("{} мс", self.interval_ms))
                .show_ui(ui, |ui| {
                    for ms in [250, 500, 1000, 2000, 5000] {
                        ui.selectable_value(&mut self.interval_ms, ms, format!("{ms} мс"));
                    }
                });
        });
        if before != self.interval_ms
            && let Some(w) = &self.worker
            && let Err(e) = w.set_interval(Duration::from_millis(self.interval_ms))
        {
            self.interval_ms = before;
            self.error = Some(e);
        }
        ui.label(format!("Пропущено уведомлений: {}", self.dropped));
        if let Some(e) = &self.error {
            ui.colored_label(ui.visuals().error_fg_color, format!("ERROR: {e}"));
        }
    }

    pub(crate) fn show_ram(&mut self, ui: &mut egui::Ui) {
        self.controls(ui);
        self.ram_history.show(ui);
        let Some(snapshot) = &self.latest else {
            if self.worker.is_some() && !self.disconnected {
                ui.label("Ожидание первого снимка RAM…");
            }
            return;
        };
        ui.label(format!(
            "Снимок #{} · UTC {}",
            snapshot.sequence,
            snapshot.captured_at.to_rfc3339()
        ));
        ui.monospace(snapshot.ram.availability().to_string());
        match &snapshot.ram {
            PlatformResult::Success(m) => {
                let info = &m.value;
                ram_metric(ui, "Общий объём (видимый ОС)", &info.total_bytes);
                ram_metric(ui, "Доступно", &info.available_bytes);
                ram_metric(ui, "Используется (общий − доступный)", &info.used_bytes);
                if let Some(f) = crate::ram::usage(info) {
                    ui.add(
                        egui::ProgressBar::new(f)
                            .text(format!("Используется: {:.1}% · CALCULATED", f * 100.0)),
                    );
                }
                ui.label(
                    "Доступная память может включать освобождаемый кэш. Объём относится к памяти, видимой ОС.",
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

    pub(crate) fn show_cpu_live(&mut self, ui: &mut egui::Ui) {
        self.controls(ui);
        self.cpu_history.show(ui);
        let Some(snapshot) = &self.latest else {
            if self.worker.is_some() && !self.disconnected {
                ui.label("Ожидание первого снимка CPU…");
            }
            return;
        };
        ui.label(format!(
            "Снимок #{} · UTC {}",
            snapshot.sequence,
            snapshot.captured_at.to_rfc3339()
        ));
        ui.monospace(snapshot.cpu.availability().to_string());
        match &snapshot.cpu {
            PlatformResult::Success(measurement) => {
                let metrics = &measurement.value;
                ui.monospace(match measurement.source {
                    DataSource::Measured => "MEASURED",
                    DataSource::Calculated => "CALCULATED",
                    DataSource::Simulated => "SIMULATED",
                });
                ui.small(format!(
                    "Интервал измерения счётчиков: {:.0} мс · UTC {}",
                    metrics.sampling_interval.as_secs_f64() * 1000.0,
                    measurement.timestamp.to_rfc3339()
                ));
                total_usage(ui, &metrics.total_usage);
                frequency_summary(ui, &metrics.logical_processors);
                logical_table(ui, &metrics.logical_processors);
                ui.add_space(8.0);
                ui.label(
                    "Нагрузка — отношение занятого времени логического процессора за окно счётчиков ОС. Частоты — текущие и максимальные значения, которые сообщает ОС. Первый снимок и слишком частые запросы дают UNAVAILABLE (прогрев / минимум интервала), а не ноль.",
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

fn ram_metric(ui: &mut egui::Ui, label: &str, result: &PlatformResult<u64>) {
    egui::Frame::group(ui.style()).show(ui, |ui| {
        ui.strong(label);
        match result {
            PlatformResult::Success(m) => {
                ui.label(format!(
                    "{} байт · {:.2} GiB",
                    m.value,
                    m.value as f64 / 1_073_741_824.0
                ));
                ui.monospace(match m.source {
                    DataSource::Measured => "MEASURED",
                    DataSource::Calculated => "CALCULATED",
                    DataSource::Simulated => "SIMULATED",
                });
                ui.small(format!("UTC {}", m.timestamp.to_rfc3339()));
            }
            PlatformResult::PermissionRequired(r)
            | PlatformResult::Unavailable(r)
            | PlatformResult::NotImplemented(r) => {
                ui.label(format!("{}: {r}", result.availability()));
            }
            PlatformResult::Error(e) => {
                ui.colored_label(ui.visuals().error_fg_color, format!("ERROR: {e}"));
            }
        }
    });
}

fn total_usage(ui: &mut egui::Ui, result: &PlatformResult<CpuUsagePercent>) {
    egui::Frame::group(ui.style()).show(ui, |ui| {
        ui.strong("Общая нагрузка");
        match result {
            PlatformResult::Success(m) => {
                let fraction = m.value.get() / 100.0;
                ui.add(
                    egui::ProgressBar::new(fraction)
                        .text(format!("{:.1}% · MEASURED", m.value.get())),
                );
                ui.small(format!("UTC {}", m.timestamp.to_rfc3339()));
            }
            PlatformResult::PermissionRequired(r)
            | PlatformResult::Unavailable(r)
            | PlatformResult::NotImplemented(r) => {
                ui.label(format!("{}: {r}", result.availability()));
            }
            PlatformResult::Error(e) => {
                ui.colored_label(ui.visuals().error_fg_color, format!("ERROR: {e}"));
            }
        }
    });
}

fn frequency_summary(ui: &mut egui::Ui, processors: &[LogicalCpuUsage]) {
    let currents: Vec<u64> = processors
        .iter()
        .filter_map(|cpu| match &cpu.current_mhz {
            PlatformResult::Success(m) => Some(m.value),
            _ => None,
        })
        .collect();
    egui::Frame::group(ui.style()).show(ui, |ui| {
        ui.strong("Частоты процессора");
        if currents.is_empty() {
            ui.monospace("UNAVAILABLE");
            ui.label("Текущие частоты логических процессоров в этом снимке недоступны.");
            return;
        }
        let min = *currents.iter().min().unwrap();
        let max = *currents.iter().max().unwrap();
        let avg = currents.iter().sum::<u64>() as f64 / currents.len() as f64;
        ui.label(format!(
            "Текущие: min {min} · avg {avg:.0} · max {max} МГц · CALCULATED по MEASURED"
        ));
        ui.small(format!(
            "Учтено {} из {} логических процессоров",
            currents.len(),
            processors.len()
        ));
    });
}

fn mhz_cell(ui: &mut egui::Ui, result: &PlatformResult<u64>) {
    match result {
        PlatformResult::Success(m) => {
            ui.label(format!("{} МГц", m.value));
        }
        other => {
            ui.monospace(other.availability().to_string());
        }
    }
}

fn logical_table(ui: &mut egui::Ui, processors: &[LogicalCpuUsage]) {
    ui.add_space(8.0);
    ui.strong("Логические процессоры");
    if processors.is_empty() {
        ui.label("Список логических процессоров в этом снимке пуст.");
        return;
    }
    egui::ScrollArea::horizontal().show(ui, |ui| {
        egui::Grid::new("cpu_logical_usage")
            .striped(true)
            .spacing([16.0, 8.0])
            .show(ui, |ui| {
                ui.label("Индекс");
                ui.label("Имя");
                ui.label("Нагрузка");
                ui.label("Текущая");
                ui.label("Макс.");
                ui.label("Статус нагрузки");
                ui.end_row();
                for cpu in processors {
                    ui.monospace(cpu.index.to_string());
                    ui.label(&cpu.name);
                    match &cpu.usage {
                        PlatformResult::Success(m) => {
                            ui.label(format!("{:.1}%", m.value.get()));
                        }
                        other => {
                            ui.monospace(other.availability().to_string());
                        }
                    }
                    mhz_cell(ui, &cpu.current_mhz);
                    mhz_cell(ui, &cpu.max_mhz);
                    ui.monospace(cpu.usage.availability().to_string());
                    ui.end_row();
                }
            });
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::UNIX_EPOCH;
    use u_monitor_core::{CpuMetrics, Measurement, RamInfo};

    fn field(value: u64) -> PlatformResult<u64> {
        PlatformResult::Success(Measurement {
            value,
            source: DataSource::Measured,
            timestamp: UNIX_EPOCH.into(),
        })
    }

    #[test]
    fn ram_and_cpu_failure_states_render_without_worker() {
        let ctx = egui::Context::default();
        let mut live = LiveTelemetry::new(Err(DiagnosticError::PlatformError("startup".into())));
        for (ram, cpu) in [
            (
                PlatformResult::Success(Measurement {
                    value: RamInfo {
                        total_bytes: field(10),
                        available_bytes: field(0),
                        used_bytes: field(10),
                    },
                    source: DataSource::Measured,
                    timestamp: UNIX_EPOCH.into(),
                }),
                PlatformResult::Unavailable("warmup".into()),
            ),
            (
                PlatformResult::Error(DiagnosticError::PlatformError("read".into())),
                PlatformResult::Error(DiagnosticError::PlatformError("cpu".into())),
            ),
            (
                PlatformResult::PermissionRequired("denied".into()),
                PlatformResult::NotImplemented("pending".into()),
            ),
        ] {
            live.latest = Some(TelemetrySnapshot {
                sequence: 1,
                captured_at: UNIX_EPOCH.into(),
                elapsed: Duration::ZERO,
                ram,
                cpu,
            });
            let output = ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    live.show_ram(ui);
                    live.show_cpu_live(ui);
                });
            });
            assert!(!output.shapes.is_empty());
        }
        assert!(live.shutdown().is_ok());
    }

    #[test]
    fn measured_cpu_snapshot_renders_total_and_logical_rows() {
        let ctx = egui::Context::default();
        let mut live = LiveTelemetry::new(Err(DiagnosticError::PlatformError("startup".into())));
        live.latest = Some(TelemetrySnapshot {
            sequence: 3,
            captured_at: UNIX_EPOCH.into(),
            elapsed: Duration::from_millis(500),
            ram: PlatformResult::NotImplemented("unused".into()),
            cpu: PlatformResult::Success(Measurement {
                value: CpuMetrics {
                    sampling_interval: Duration::from_millis(250),
                    total_usage: PlatformResult::Success(Measurement {
                        value: CpuUsagePercent::try_from(37.5).unwrap(),
                        source: DataSource::Measured,
                        timestamp: UNIX_EPOCH.into(),
                    }),
                    logical_processors: vec![LogicalCpuUsage {
                        index: 0,
                        name: "cpu0".into(),
                        usage: PlatformResult::Success(Measurement {
                            value: CpuUsagePercent::try_from(12.0).unwrap(),
                            source: DataSource::Measured,
                            timestamp: UNIX_EPOCH.into(),
                        }),
                        current_mhz: PlatformResult::Success(Measurement {
                            value: 3200,
                            source: DataSource::Measured,
                            timestamp: UNIX_EPOCH.into(),
                        }),
                        max_mhz: PlatformResult::Success(Measurement {
                            value: 4800,
                            source: DataSource::Measured,
                            timestamp: UNIX_EPOCH.into(),
                        }),
                    }],
                },
                source: DataSource::Measured,
                timestamp: UNIX_EPOCH.into(),
            }),
        });
        let mut text = String::new();
        for _ in 0..2 {
            let output = ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| live.show_cpu_live(ui));
            });
            text.clear();
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
            for shape in output.shapes {
                collect(&shape.shape, &mut text);
            }
        }
        for expected in [
            "37.5%",
            "cpu0",
            "12.0%",
            "3200 МГц",
            "4800 МГц",
            "SUPPORTED",
        ] {
            assert!(text.contains(expected), "missing {expected}: {text}");
        }
    }
}

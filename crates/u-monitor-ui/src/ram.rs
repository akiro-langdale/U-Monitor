use eframe::egui;
use std::{sync::mpsc::TryRecvError, time::Duration};
use u_monitor_core::{DataSource, DiagnosticError, PlatformResult, RamInfo};
use u_monitor_telemetry::{TelemetrySnapshot, TelemetryWorker};

mod history;
use history::RamHistory;

pub(crate) struct RamPanel {
    worker: Option<TelemetryWorker>,
    latest: Option<TelemetrySnapshot>,
    history: RamHistory,
    error: Option<DiagnosticError>,
    disconnected: bool,
    interval_ms: u64,
    dropped: u64,
}
impl RamPanel {
    pub(crate) fn new(worker: Result<TelemetryWorker, DiagnosticError>) -> Self {
        let (worker, error) = match worker {
            Ok(w) => (Some(w), None),
            Err(e) => (None, Some(e)),
        };
        Self {
            worker,
            error,
            latest: None,
            history: RamHistory::default(),
            disconnected: false,
            interval_ms: 1000,
            dropped: 0,
        }
    }
    pub(crate) fn poll(&mut self, ctx: &egui::Context) {
        if let Some(worker) = self.worker.as_ref().filter(|_| !self.disconnected) {
            // Bound work per frame; never wait for a snapshot here.
            for _ in 0..8 {
                match worker.try_recv() {
                    Ok(snapshot) => {
                        self.history.push(&snapshot);
                        self.latest = Some(snapshot);
                    }
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => {
                        self.disconnected = true;
                        self.error = Some(DiagnosticError::PlatformError(
                            "Поток RAM завершён. Ниже — последний полученный снимок.".into(),
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
    pub(crate) fn show(&mut self, ui: &mut egui::Ui) {
        let before = self.interval_ms;
        ui.add_enabled_ui(self.worker.is_some() && !self.disconnected, |ui| {
            egui::ComboBox::from_id_salt("ram_interval")
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
        self.history.show(ui);
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
                metric(ui, "Общий объём (видимый ОС)", &info.total_bytes);
                metric(ui, "Доступно", &info.available_bytes);
                metric(ui, "Используется (общий − доступный)", &info.used_bytes);
                if let Some(f) = usage(info) {
                    ui.add(
                        egui::ProgressBar::new(f)
                            .text(format!("Используется: {:.1}% · CALCULATED", f * 100.0)),
                    );
                }
                ui.label("Доступная память может включать освобождаемый кэш. Объём относится к памяти, видимой ОС.");
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
fn metric(ui: &mut egui::Ui, label: &str, result: &PlatformResult<u64>) {
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
fn usage(info: &RamInfo) -> Option<f32> {
    match (&info.total_bytes, &info.used_bytes) {
        (PlatformResult::Success(t), PlatformResult::Success(u))
            if t.value > 0 && u.value <= t.value =>
        {
            Some((u.value as f64 / t.value as f64) as f32)
        }
        _ => None,
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::time::UNIX_EPOCH;
    use u_monitor_core::Measurement;
    fn field(value: u64) -> PlatformResult<u64> {
        PlatformResult::Success(Measurement {
            value,
            source: DataSource::Measured,
            timestamp: UNIX_EPOCH.into(),
        })
    }
    #[test]
    fn progress_requires_valid_metrics() {
        let mut info = RamInfo {
            total_bytes: field(10),
            available_bytes: field(5),
            used_bytes: field(5),
        };
        assert_eq!(usage(&info), Some(0.5));
        for (total, used) in [(0, 0), (10, 11)] {
            info.total_bytes = field(total);
            info.used_bytes = field(used);
            assert!(usage(&info).is_none());
        }
        info.used_bytes = PlatformResult::Unavailable("missing".into());
        assert!(usage(&info).is_none());
    }
    #[test]
    fn errors_and_permission_states_render_without_previous_values() {
        let ctx = egui::Context::default();
        let mut panel = RamPanel::new(Err(DiagnosticError::PlatformError("startup".into())));
        for ram in [
            PlatformResult::Success(Measurement {
                value: RamInfo {
                    total_bytes: field(10),
                    available_bytes: field(0),
                    used_bytes: field(10),
                },
                source: DataSource::Measured,
                timestamp: UNIX_EPOCH.into(),
            }),
            PlatformResult::Error(DiagnosticError::PlatformError("read".into())),
            PlatformResult::PermissionRequired("denied".into()),
            PlatformResult::Unavailable("missing".into()),
            PlatformResult::NotImplemented("pending".into()),
        ] {
            panel.latest = Some(TelemetrySnapshot {
                cpu: PlatformResult::NotImplemented("test fixture".into()),
                sequence: 1,
                captured_at: UNIX_EPOCH.into(),
                elapsed: Duration::ZERO,
                ram,
            });
            let output = ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| panel.show(ui));
            });
            assert!(!output.shapes.is_empty());
        }
        assert!(panel.shutdown().is_ok());
    }
}

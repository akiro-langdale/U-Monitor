//! Bounded view of received CPU total-usage notifications.
use eframe::egui;
use std::{collections::VecDeque, time::Duration};
use u_monitor_core::{PlatformResult, Timestamp};
use u_monitor_telemetry::TelemetrySnapshot;

const CAPACITY: usize = 60;

struct Sample {
    sequence: u64,
    elapsed: Duration,
    captured_at: Timestamp,
    /// Fraction 0..=1 derived from measured percent. Missing data stays `None`.
    usage: Option<f32>,
}

impl Sample {
    fn connects_to(&self, next: &Self) -> bool {
        self.sequence.checked_add(1) == Some(next.sequence)
            && self.elapsed < next.elapsed
            && self.usage.is_some()
            && next.usage.is_some()
    }
}

#[derive(Default)]
pub(crate) struct CpuHistory {
    samples: VecDeque<Sample>,
}

impl CpuHistory {
    pub(crate) fn push(&mut self, snapshot: &TelemetrySnapshot) {
        let usage = match &snapshot.cpu {
            PlatformResult::Success(measurement) => match &measurement.value.total_usage {
                PlatformResult::Success(percent) => Some(percent.value.get() / 100.0),
                _ => None,
            },
            _ => None,
        };
        if self.samples.len() == CAPACITY {
            self.samples.pop_front();
        }
        self.samples.push_back(Sample {
            sequence: snapshot.sequence,
            elapsed: snapshot.elapsed,
            captured_at: snapshot.captured_at,
            usage,
        });
    }

    pub(crate) fn show(&self, ui: &mut egui::Ui) {
        ui.add_space(8.0);
        ui.strong("История общей нагрузки CPU · MEASURED");
        ui.small(format!(
            "Последние {CAPACITY} полученных снимков · {} в истории",
            self.samples.len()
        ));
        let Some(last) = self.samples.back() else {
            ui.label("График появится после первого снимка CPU.");
            return;
        };
        let span = last
            .elapsed
            .saturating_sub(self.samples[0].elapsed)
            .as_secs_f64()
            .max(1.0);
        let (response, painter) = ui.allocate_painter(
            egui::vec2(ui.available_width().max(1.0), 200.0),
            egui::Sense::hover(),
        );
        let rect = response.rect;
        let plot = egui::Rect::from_min_max(
            rect.min + egui::vec2(42.0, 10.0),
            rect.max - egui::vec2(12.0, 28.0),
        );
        if plot.width() <= 0.0 {
            return;
        }
        let text_color = ui.visuals().text_color();
        let color = ui.visuals().selection.bg_fill;
        let font = egui::FontId::proportional(11.0);
        for percent in [0, 50, 100] {
            let y = plot.bottom() - plot.height() * percent as f32 / 100.0;
            painter.line_segment(
                [egui::pos2(plot.left(), y), egui::pos2(plot.right(), y)],
                ui.visuals().widgets.noninteractive.bg_stroke,
            );
            painter.text(
                egui::pos2(plot.left() - 5.0, y),
                egui::Align2::RIGHT_CENTER,
                format!("{percent}%"),
                font.clone(),
                text_color,
            );
        }
        for (x, label, align) in [
            (plot.left(), format!("−{span:.1} с"), egui::Align2::LEFT_TOP),
            (
                plot.right(),
                "Последний снимок".into(),
                egui::Align2::RIGHT_TOP,
            ),
        ] {
            painter.text(
                egui::pos2(x, plot.bottom() + 6.0),
                align,
                label,
                font.clone(),
                text_color,
            );
        }
        let x_of = |sample: &Sample| {
            let age = last.elapsed.saturating_sub(sample.elapsed).as_secs_f64();
            plot.right() - (age / span) as f32 * plot.width()
        };
        let position = |sample: &Sample, usage: f32| {
            egui::pos2(x_of(sample), plot.bottom() - usage * plot.height())
        };
        let mut previous: Option<&Sample> = None;
        for sample in &self.samples {
            if let Some(value) = sample.usage {
                let point = position(sample, value);
                if let Some(prev) = previous.filter(|prev| prev.connects_to(sample)) {
                    painter.line_segment(
                        [position(prev, prev.usage.unwrap()), point],
                        egui::Stroke::new(2.0, color),
                    );
                }
                painter.circle_filled(point, 2.5, color);
            }
            previous = Some(sample);
        }
        if let Some(pointer) = response.hover_pos().filter(|pos| plot.contains(*pos)) {
            let nearest = self.samples.iter().min_by(|a, b| {
                (x_of(a) - pointer.x)
                    .abs()
                    .total_cmp(&(x_of(b) - pointer.x).abs())
            });
            if let Some(sample) = nearest {
                painter.line_segment(
                    [
                        egui::pos2(x_of(sample), plot.top()),
                        egui::pos2(x_of(sample), plot.bottom()),
                    ],
                    egui::Stroke::new(1.0, text_color),
                );
                response.show_tooltip_ui(|ui| {
                    ui.label(format!(
                        "Снимок #{} · UTC {}",
                        sample.sequence,
                        sample.captured_at.to_rfc3339()
                    ));
                    match sample.usage {
                        Some(value) => {
                            ui.label(format!("Нагрузка: {:.1}% · MEASURED", value * 100.0));
                        }
                        None => {
                            ui.label("Нет достоверного значения CPU");
                        }
                    }
                });
            }
        }
        ui.small(
            "Время относительно последнего снимка. Прогрев, ошибки и пропуски разрывают линию.",
        );
        if self.samples.iter().all(|sample| sample.usage.is_none()) {
            ui.label("В истории пока нет достоверных значений нагрузки CPU.");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::UNIX_EPOCH;
    use u_monitor_core::{
        CpuMetrics, CpuUsagePercent, DataSource, DiagnosticError, Measurement, RamInfo,
    };

    fn ram_stub() -> PlatformResult<RamInfo> {
        PlatformResult::NotImplemented("test fixture".into())
    }

    fn snapshot(sequence: u64, millis: u64, percent: f32) -> TelemetrySnapshot {
        TelemetrySnapshot {
            sequence,
            elapsed: Duration::from_millis(millis),
            captured_at: UNIX_EPOCH.into(),
            ram: ram_stub(),
            cpu: PlatformResult::Success(Measurement {
                value: CpuMetrics {
                    sampling_interval: Duration::from_millis(250),
                    total_usage: PlatformResult::Success(Measurement {
                        value: CpuUsagePercent::try_from(percent).unwrap(),
                        source: DataSource::Measured,
                        timestamp: UNIX_EPOCH.into(),
                    }),
                    logical_processors: Vec::new(),
                },
                source: DataSource::Measured,
                timestamp: UNIX_EPOCH.into(),
            }),
            disk: PlatformResult::NotImplemented("test fixture".into()),
        }
    }

    #[test]
    fn retention_evicts_oldest_including_warmup_failures() {
        let mut history = CpuHistory::default();
        for seq in 1..=61 {
            let mut sample = snapshot(seq, seq * 250, 40.0);
            if seq == 61 {
                sample.cpu = PlatformResult::Unavailable("warmup".into());
            }
            history.push(&sample);
        }
        assert_eq!(history.samples.len(), CAPACITY);
        assert_eq!(history.samples.front().unwrap().sequence, 2);
        assert_eq!(history.samples.back().unwrap().usage, None);
    }

    #[test]
    fn gaps_and_failures_break_lines_without_hiding_valid_zero() {
        let mut history = CpuHistory::default();
        history.push(&snapshot(1, 0, 0.0));
        history.push(&snapshot(2, 250, 50.0));
        history.push(&snapshot(4, 750, 60.0));
        let mut failed = snapshot(5, 1000, 0.0);
        failed.cpu = PlatformResult::Error(DiagnosticError::Unknown);
        history.push(&failed);
        history.push(&snapshot(6, 1250, 70.0));
        assert_eq!(history.samples[0].usage, Some(0.0));
        let connections: Vec<_> = history
            .samples
            .iter()
            .zip(history.samples.iter().skip(1))
            .map(|(a, b)| a.connects_to(b))
            .collect();
        assert_eq!(connections, [true, false, false, false]);
    }

    #[test]
    fn empty_and_failed_histories_render() {
        let ctx = egui::Context::default();
        let mut history = CpuHistory::default();
        for sample in [None, Some(snapshot(1, 0, 12.5))] {
            if let Some(sample) = sample {
                history.push(&sample);
            }
            let output = ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| history.show(ui));
            });
            assert!(!output.shapes.is_empty());
        }
    }
}

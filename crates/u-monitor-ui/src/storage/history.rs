//! Bounded disk I/O rate history from shared telemetry notifications.
use eframe::egui;
use std::{collections::VecDeque, time::Duration};
use u_monitor_core::{PlatformResult, Timestamp};
use u_monitor_telemetry::TelemetrySnapshot;

const CAPACITY: usize = 60;

struct Sample {
    sequence: u64,
    elapsed: Duration,
    captured_at: Timestamp,
    read_bps: Option<f64>,
    write_bps: Option<f64>,
}

impl Sample {
    fn connects_to(&self, next: &Self) -> bool {
        self.sequence.checked_add(1) == Some(next.sequence)
            && self.elapsed < next.elapsed
            && self.read_bps.is_some()
            && next.read_bps.is_some()
            && self.write_bps.is_some()
            && next.write_bps.is_some()
    }
}

#[derive(Default)]
pub(crate) struct DiskIoHistory {
    samples: VecDeque<Sample>,
}

impl DiskIoHistory {
    pub(crate) fn push(&mut self, snapshot: &TelemetrySnapshot) {
        let (read_bps, write_bps) = match &snapshot.disk {
            PlatformResult::Success(measurement) => (
                match &measurement.value.read_bytes_per_sec {
                    PlatformResult::Success(value) => Some(value.value),
                    _ => None,
                },
                match &measurement.value.write_bytes_per_sec {
                    PlatformResult::Success(value) => Some(value.value),
                    _ => None,
                },
            ),
            _ => (None, None),
        };
        if self.samples.len() == CAPACITY {
            self.samples.pop_front();
        }
        self.samples.push_back(Sample {
            sequence: snapshot.sequence,
            elapsed: snapshot.elapsed,
            captured_at: snapshot.captured_at,
            read_bps,
            write_bps,
        });
    }

    pub(crate) fn show(&self, ui: &mut egui::Ui) {
        ui.add_space(8.0);
        ui.strong("История Disk I/O · CALCULATED bytes/s");
        ui.small(format!(
            "Последние {CAPACITY} полученных снимков · {} в истории",
            self.samples.len()
        ));
        let Some(last) = self.samples.back() else {
            ui.label("График появится после первого успешного сэмпла диска.");
            return;
        };
        let span = last
            .elapsed
            .saturating_sub(self.samples[0].elapsed)
            .as_secs_f64()
            .max(1.0);
        let max_rate = self
            .samples
            .iter()
            .flat_map(|sample| [sample.read_bps, sample.write_bps])
            .flatten()
            .fold(1.0_f64, f64::max)
            .max(1.0);
        let (response, painter) = ui.allocate_painter(
            egui::vec2(ui.available_width().max(1.0), 160.0),
            egui::Sense::hover(),
        );
        let rect = response.rect;
        let plot = egui::Rect::from_min_max(
            rect.min + egui::vec2(56.0, 10.0),
            rect.max - egui::vec2(12.0, 28.0),
        );
        if plot.width() <= 0.0 {
            return;
        }
        let text_color = ui.visuals().text_color();
        let read_color = ui.visuals().selection.bg_fill;
        let write_color = ui.visuals().warn_fg_color;
        let font = egui::FontId::proportional(11.0);
        for fraction in [0.0_f32, 0.5, 1.0] {
            let y = plot.bottom() - plot.height() * fraction;
            painter.line_segment(
                [egui::pos2(plot.left(), y), egui::pos2(plot.right(), y)],
                ui.visuals().widgets.noninteractive.bg_stroke,
            );
            painter.text(
                egui::pos2(plot.left() - 5.0, y),
                egui::Align2::RIGHT_CENTER,
                format_bps(max_rate * f64::from(fraction)),
                font.clone(),
                text_color,
            );
        }
        paint_series(
            &painter,
            &self.samples,
            plot,
            span,
            max_rate,
            |sample| sample.read_bps,
            read_color,
        );
        paint_series(
            &painter,
            &self.samples,
            plot,
            span,
            max_rate,
            |sample| sample.write_bps,
            write_color,
        );
        painter.text(
            egui::pos2(plot.left(), plot.bottom() + 4.0),
            egui::Align2::LEFT_TOP,
            format!(
                "−{span:.1} с · read/write · UTC {}",
                last.captured_at.to_rfc3339()
            ),
            font,
            text_color,
        );
        ui.horizontal(|ui| {
            ui.colored_label(read_color, "read");
            ui.colored_label(write_color, "write");
        });
    }
}

fn paint_series(
    painter: &egui::Painter,
    samples: &VecDeque<Sample>,
    plot: egui::Rect,
    span: f64,
    max_rate: f64,
    value: impl Fn(&Sample) -> Option<f64>,
    color: egui::Color32,
) {
    let origin = samples
        .front()
        .map(|sample| sample.elapsed)
        .unwrap_or_default();
    for window in samples.iter().collect::<Vec<_>>().windows(2) {
        let [left, right] = window else { continue };
        if !left.connects_to(right) {
            continue;
        }
        let (Some(left_value), Some(right_value)) = (value(left), value(right)) else {
            continue;
        };
        let x0 = plot.left()
            + plot.width() * ((left.elapsed - origin).as_secs_f64() / span).clamp(0.0, 1.0) as f32;
        let x1 = plot.left()
            + plot.width() * ((right.elapsed - origin).as_secs_f64() / span).clamp(0.0, 1.0) as f32;
        let y0 = plot.bottom() - plot.height() * (left_value / max_rate).clamp(0.0, 1.0) as f32;
        let y1 = plot.bottom() - plot.height() * (right_value / max_rate).clamp(0.0, 1.0) as f32;
        painter.line_segment(
            [egui::pos2(x0, y0), egui::pos2(x1, y1)],
            egui::Stroke::new(1.5, color),
        );
    }
}

fn format_bps(value: f64) -> String {
    if value >= 1_048_576.0 {
        format!("{:.1} MiB/s", value / 1_048_576.0)
    } else if value >= 1024.0 {
        format!("{:.1} KiB/s", value / 1024.0)
    } else {
        format!("{value:.0} B/s")
    }
}

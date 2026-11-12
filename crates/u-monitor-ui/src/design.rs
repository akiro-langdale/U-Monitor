use eframe::egui::{self, Color32, RichText, Stroke};
use u_monitor_core::DataSource;

fn rgb(value: u32) -> Color32 {
    Color32::from_rgb((value >> 16) as u8, (value >> 8) as u8, value as u8)
}

pub(crate) fn visuals(dark: bool) -> egui::Visuals {
    let mut v = if dark {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };
    let (canvas, surface, field, border, text, muted, active, hover, selected, accent) = if dark {
        (
            0x0c141a, 0x131f28, 0x0a1218, 0x2b3e49, 0xe4eff2, 0xa0b5bf, 0x1b2c36, 0x243d48,
            0x163f42, 0x5fe1c7,
        )
    } else {
        (
            0xeef3f5, 0xffffff, 0xf6f9fa, 0xc4d2d9, 0x172d38, 0x4b6370, 0xe5eef1, 0xd8eaed,
            0xd0ede6, 0x075e55,
        )
    };
    v.panel_fill = rgb(canvas);
    v.window_fill = rgb(surface);
    v.extreme_bg_color = rgb(field);
    v.faint_bg_color = rgb(active);
    v.code_bg_color = rgb(field);
    v.window_stroke = Stroke::new(1.0, rgb(border));
    v.override_text_color = None;
    v.weak_text_color = Some(rgb(muted));
    v.hyperlink_color = rgb(accent);
    v.selection.bg_fill = rgb(selected);
    v.selection.stroke = Stroke::new(1.0, rgb(accent));

    for (widget, fill, foreground) in [
        (&mut v.widgets.noninteractive, surface, text),
        (&mut v.widgets.inactive, active, text),
        (&mut v.widgets.hovered, hover, text),
        (&mut v.widgets.active, selected, text),
        (&mut v.widgets.open, hover, text),
    ] {
        widget.bg_fill = rgb(fill);
        widget.weak_bg_fill = rgb(fill);
        widget.bg_stroke = Stroke::new(1.0, rgb(border));
        widget.fg_stroke.color = rgb(foreground);
        widget.corner_radius = 5.into();
        widget.expansion = 0.0;
    }
    v
}

pub(crate) fn surface(ui: &egui::Ui) -> egui::Frame {
    egui::Frame::new()
        .fill(ui.visuals().window_fill)
        .stroke(ui.visuals().window_stroke)
        .corner_radius(8)
        .inner_margin(14)
}

pub(crate) fn section(ui: &mut egui::Ui, title: &str) {
    ui.add_space(10.0);
    ui.label(RichText::new(title).size(18.0).strong());
    ui.add_space(3.0);
}

pub(crate) fn source(ui: &mut egui::Ui, source: DataSource) {
    let (label, dark, light) = match source {
        DataSource::Measured => ("MEASURED", 0x5fe1c7, 0x075e55),
        DataSource::Calculated => ("CALCULATED", 0xffcb7a, 0x79500a),
        DataSource::Simulated => ("SIMULATED", 0xc9afff, 0x6637a2),
    };
    let color = rgb(if ui.visuals().dark_mode { dark } else { light });
    egui::Frame::new()
        .fill(ui.visuals().faint_bg_color)
        .stroke(Stroke::new(1.0, color.gamma_multiply(0.45)))
        .corner_radius(4)
        .inner_margin(egui::Margin::symmetric(7, 2))
        .show(ui, |ui| {
            ui.label(
                RichText::new(label)
                    .size(11.0)
                    .monospace()
                    .strong()
                    .color(color),
            );
        });
}

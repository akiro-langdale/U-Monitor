//! Process/thread enumeration and inspection-target selection.
use eframe::egui;
use u_monitor_core::{InspectionTarget, PlatformResult, ProcessInfo, ThreadInfo};

pub(crate) struct ProcessesPanel {
    processes: PlatformResult<Vec<ProcessInfo>>,
    filter: String,
    selected_pid: Option<u64>,
    threads: PlatformResult<Vec<ThreadInfo>>,
    target: InspectionTarget,
}

impl ProcessesPanel {
    pub(crate) fn new(
        processes: PlatformResult<Vec<ProcessInfo>>,
        target: InspectionTarget,
    ) -> Self {
        Self {
            processes,
            filter: String::new(),
            selected_pid: Some(target.process.pid),
            threads: PlatformResult::NotImplemented("select a process".into()),
            target,
        }
    }

    pub(crate) fn target(&self) -> &InspectionTarget {
        &self.target
    }

    pub(crate) fn show(
        &mut self,
        ui: &mut egui::Ui,
        refresh_processes: &dyn Fn() -> PlatformResult<Vec<ProcessInfo>>,
        list_threads: &dyn Fn(u64) -> PlatformResult<Vec<ThreadInfo>>,
    ) {
        ui.label(
            "Выберите процесс, затем откройте раздел «Память» для просмотра доступных регионов.",
        );
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if ui.button("Обновить список").clicked() {
                self.processes = refresh_processes();
            }
            ui.label("Фильтр");
            ui.text_edit_singleline(&mut self.filter);
        });
        ui.add_space(8.0);
        ui.monospace(format!(
            "Цель: pid {} ({}){}",
            self.target.process.pid,
            self.target.process.name,
            if self.target.is_current_process() {
                " · текущий"
            } else {
                " · чужой"
            }
        ));
        if let Some(thread) = &self.target.thread {
            ui.monospace(format!("Поток: tid {}", thread.tid));
        }

        ui.add_space(12.0);
        match &self.processes {
            PlatformResult::Success(measurement) => {
                crate::design::source(ui, measurement.source);
                let filter = self.filter.to_ascii_lowercase();
                egui::ScrollArea::vertical()
                    .id_salt("process_list")
                    .max_height(280.0)
                    .show(ui, |ui| {
                        egui::Grid::new("process_grid")
                            .striped(true)
                            .spacing([12.0, 4.0])
                            .show(ui, |ui| {
                                ui.label("PID");
                                ui.label("Имя");
                                ui.label("");
                                ui.end_row();
                                for process in &measurement.value {
                                    if !filter.is_empty()
                                        && !process.name.to_ascii_lowercase().contains(&filter)
                                        && !process.pid.to_string().contains(&filter)
                                    {
                                        continue;
                                    }
                                    let selected = self.selected_pid == Some(process.pid);
                                    ui.monospace(process.pid.to_string());
                                    ui.label(if process.is_current {
                                        format!("{} (этот)", process.name)
                                    } else {
                                        process.name.clone()
                                    });
                                    if ui
                                        .selectable_label(
                                            selected,
                                            if selected { "●" } else { "○" },
                                        )
                                        .clicked()
                                    {
                                        self.selected_pid = Some(process.pid);
                                        self.threads = list_threads(process.pid);
                                    }
                                    ui.end_row();
                                }
                            });
                    });
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

        ui.add_space(12.0);
        crate::design::section(ui, "Потоки");
        match &self.threads {
            PlatformResult::Success(measurement) => {
                egui::ScrollArea::vertical()
                    .id_salt("thread_list")
                    .max_height(180.0)
                    .show(ui, |ui| {
                        for thread in &measurement.value {
                            ui.horizontal(|ui| {
                                ui.monospace(format!("tid {}", thread.tid));
                                if let Some(name) = &thread.name {
                                    ui.label(name);
                                }
                                if ui.button("Цель").clicked()
                                    && let PlatformResult::Success(processes) = &self.processes
                                    && let Some(process) = processes
                                        .value
                                        .iter()
                                        .find(|process| process.pid == thread.process_pid)
                                        .cloned()
                                {
                                    self.target = InspectionTarget {
                                        process,
                                        thread: Some(thread.clone()),
                                    };
                                }
                            });
                        }
                    });
            }
            PlatformResult::NotImplemented(r) => {
                ui.label(r);
            }
            PlatformResult::PermissionRequired(r) => {
                ui.monospace("PERMISSION_REQUIRED");
                ui.label(r);
            }
            PlatformResult::Unavailable(r) => {
                ui.label(r);
            }
            PlatformResult::Error(e) => {
                ui.colored_label(ui.visuals().error_fg_color, e.to_string());
            }
        }

        ui.add_space(8.0);
        if ui.button("Сделать выбранный процесс целью").clicked()
            && let (Some(pid), PlatformResult::Success(processes)) =
                (self.selected_pid, &self.processes)
            && let Some(process) = processes.value.iter().find(|p| p.pid == pid).cloned()
        {
            self.target = InspectionTarget {
                process,
                thread: None,
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn panel_renders_process_list() {
        let ctx = egui::Context::default();
        let mut panel = ProcessesPanel::new(
            PlatformResult::Success(u_monitor_core::Measurement {
                value: vec![ProcessInfo {
                    pid: 1,
                    name: "init".into(),
                    path: None,
                    is_current: true,
                }],
                source: u_monitor_core::DataSource::Measured,
                timestamp: std::time::UNIX_EPOCH.into(),
            }),
            InspectionTarget::current_process(1, "init"),
        );
        let output = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                panel.show(ui, &|| PlatformResult::NotImplemented("x".into()), &|_| {
                    PlatformResult::NotImplemented("x".into())
                });
            });
        });
        assert!(!output.shapes.is_empty());
    }
}

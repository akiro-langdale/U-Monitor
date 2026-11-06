use crate::{panels, ram::RamPanel};

const THEME_KEY: &str = "u-monitor.ui.theme";

fn restored_dark(storage: Option<&dyn eframe::Storage>) -> bool {
    // Missing or unrecognized settings use the application's default theme.
    !matches!(
        storage
            .and_then(|storage| storage.get_string(THEME_KEY))
            .as_deref(),
        Some("light")
    )
}
use eframe::egui;
use u_monitor_core::{PlatformResult, SystemIdentity};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Page {
    Overview,
    Cpu,
    Ram,
    Storage,
    Registers,
    Cache,
    Simulation,
}

impl Page {
    pub(crate) const ALL: [Self; 7] = [
        Self::Overview,
        Self::Cpu,
        Self::Ram,
        Self::Storage,
        Self::Registers,
        Self::Cache,
        Self::Simulation,
    ];
    pub(crate) fn title(self) -> &'static str {
        match self {
            Self::Overview => "Обзор",
            Self::Cpu => "CPU",
            Self::Ram => "RAM",
            Self::Storage => "Накопители",
            Self::Registers => "Регистры",
            Self::Cache => "Кэш",
            Self::Simulation => "Симуляция",
        }
    }
    pub(crate) fn description(self) -> &'static str {
        match self {
            Self::Overview => "Система и доступность разделов",
            Self::Cpu => "Модель, топология и нагрузка процессора",
            Self::Ram => "Статистика оперативной памяти",
            Self::Storage => "Тома, файловые системы и пространство",
            Self::Registers => "Снимки регистров выбранного потока",
            Self::Cache => "Характеристики и иерархия кэшей",
            Self::Simulation => "Отдельные модели иерархии памяти",
        }
    }
    pub(crate) fn pending(self) -> &'static str {
        match self {
            Self::Overview => "",
            Self::Cpu => {
                "Панель CPU ещё не подключена. Обнаружение модели и числа ядер уже реализовано в библиотеке."
            }
            Self::Ram => {
                "Панель RAM ещё не подключена. Фоновый сбор и история уже реализованы; подключение к GUI — следующий подшаг"
            }
            Self::Storage => {
                "Панель накопителей ещё не подключена. Обнаружение смонтированных томов уже реализовано в библиотеке."
            }
            Self::Registers => "Чтение регистров выбранного потока ещё не реализовано.",
            Self::Cache => "Обнаружение топологии и счётчиков кэша ещё не реализовано.",
            Self::Simulation => {
                "Модели ещё не реализованы. Будущие результаты будут явно помечены SIMULATED."
            }
        }
    }
}

pub(crate) struct MonitorApp {
    page: Page,
    dark: bool,
    identity: PlatformResult<SystemIdentity>,
    ram: RamPanel,
}

impl MonitorApp {
    pub(crate) fn new(
        cc: &eframe::CreationContext<'_>,
        identity: PlatformResult<SystemIdentity>,
        ram: RamPanel,
    ) -> Self {
        let dark = restored_dark(cc.storage);
        cc.egui_ctx.set_theme(if dark {
            egui::Theme::Dark
        } else {
            egui::Theme::Light
        });
        Self {
            page: Page::Overview,
            dark,
            identity,
            ram,
        }
    }

    // No OS queries, worker startup or blocking receives inside a UI frame.
    fn render(&mut self, ctx: &egui::Context) {
        self.ram.poll(ctx);
        egui::TopBottomPanel::top("header").show(ctx, |ui| {
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.heading("U-Monitor");
                ui.label(
                    egui::RichText::new(env!("CARGO_PKG_VERSION"))
                        .monospace()
                        .weak(),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let before = self.dark;
                    ui.selectable_value(&mut self.dark, false, "Светлая");
                    ui.selectable_value(&mut self.dark, true, "Тёмная");
                    ui.label("Тема:");
                    if before != self.dark {
                        ctx.set_theme(if self.dark {
                            egui::Theme::Dark
                        } else {
                            egui::Theme::Light
                        });
                    }
                });
            });
            ui.add_space(8.0);
        });
        egui::SidePanel::left("navigation")
            .default_width(180.0)
            .min_width(150.0)
            .show(ctx, |ui| {
                ui.add_space(16.0);
                ui.label(egui::RichText::new("ИССЛЕДОВАНИЕ СИСТЕМЫ").small().weak());
                ui.add_space(12.0);
                for page in Page::ALL {
                    ui.add_space(4.0);
                    ui.selectable_value(&mut self.page, page, page.title());
                }
                ui.add_space(24.0);
                ui.separator();
                ui.label(egui::RichText::new("Этап 05.2 · RAM").small().weak());
            });
        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.add_space(16.0);
                ui.heading(self.page.title());
                ui.label(egui::RichText::new(self.page.description()).weak());
                ui.add_space(20.0);
                match self.page {
                    Page::Overview => panels::overview(ui, &self.identity),
                    Page::Ram => self.ram.show(ui),
                    page => panels::pending(ui, page),
                }
            });
        });
    }
}

impl eframe::App for MonitorApp {
    fn on_exit(&mut self, _: Option<&eframe::glow::Context>) {
        if let Err(error) = self.ram.shutdown() {
            eprintln!("RAM shutdown failed: {error}");
        }
    }
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        storage.set_string(THEME_KEY, if self.dark { "dark" } else { "light" }.into());
    }

    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        self.render(ctx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::UNIX_EPOCH;
    use u_monitor_core::{Architecture, DataSource, DiagnosticError, Measurement, OperatingSystem};

    #[test]
    fn all_pages_render_with_identity_failures_and_unknown_targets() {
        let cases = [
            PlatformResult::Success(Measurement {
                value: SystemIdentity {
                    operating_system: OperatingSystem::Unknown("future-os".into()),
                    process_architecture: Architecture::Unknown("future-arch".into()),
                },
                source: DataSource::Measured,
                timestamp: UNIX_EPOCH.into(),
            }),
            PlatformResult::PermissionRequired("denied".into()),
            PlatformResult::Unavailable("no data".into()),
            PlatformResult::NotImplemented("pending".into()),
            PlatformResult::Error(DiagnosticError::PlatformError("failed".into())),
        ];
        for identity in cases {
            for page in Page::ALL {
                for dark in [false, true] {
                    let ctx = egui::Context::default();
                    ctx.set_theme(if dark {
                        egui::Theme::Dark
                    } else {
                        egui::Theme::Light
                    });
                    let mut app = MonitorApp {
                        page,
                        dark,
                        identity: identity.clone(),
                        ram: RamPanel::new(Err(DiagnosticError::PlatformError("test".into()))),
                    };
                    let output = ctx.run(egui::RawInput::default(), |ctx| app.render(ctx));
                    assert!(!output.shapes.is_empty());
                }
            }
        }
    }

    #[derive(Default)]
    struct MemoryStorage(std::collections::BTreeMap<String, String>);

    impl eframe::Storage for MemoryStorage {
        fn get_string(&self, key: &str) -> Option<String> {
            self.0.get(key).cloned()
        }
        fn set_string(&mut self, key: &str, value: String) {
            self.0.insert(key.into(), value);
        }
        fn flush(&mut self) {}
    }

    #[test]
    fn selected_theme_survives_app_save_and_restore() {
        for dark in [false, true] {
            let mut storage = MemoryStorage::default();
            let mut app = MonitorApp {
                page: Page::Overview,
                dark,
                identity: PlatformResult::NotImplemented("test fixture".into()),
                ram: RamPanel::new(Err(DiagnosticError::PlatformError("test".into()))),
            };
            eframe::App::save(&mut app, &mut storage);
            assert_eq!(restored_dark(Some(&storage)), dark);
        }
    }

    #[test]
    fn absent_or_unknown_saved_theme_uses_dark_default() {
        assert!(restored_dark(None));
        let mut storage = MemoryStorage::default();
        assert!(restored_dark(Some(&storage)));
        eframe::Storage::set_string(&mut storage, THEME_KEY, "unsupported".into());
        assert!(restored_dark(Some(&storage)));
    }
}

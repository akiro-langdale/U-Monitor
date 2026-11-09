use crate::{
    cache, cpu, live::LiveTelemetry, memory::MemoryPanel, panels, processes::ProcessesPanel,
    registers::RegistersPanel, simulation::SimulationPanel, storage::StoragePanel,
};

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
use u_monitor_core::{
    Availability, CacheCounters, CacheTopology, CpuInfo, FileBytes, FileReadRequest,
    InspectionTarget, MemoryBytes, MemoryReadRequest, PlatformFeature, PlatformResult, ProcessInfo,
    RegisterSnapshot, StorageVolume, SystemIdentity, ThreadInfo, VirtualMemoryRegion,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Page {
    Overview,
    Processes,
    Cpu,
    Ram,
    Memory,
    Storage,
    Registers,
    Cache,
    Simulation,
}

impl Page {
    pub(crate) const ALL: [Self; 9] = [
        Self::Overview,
        Self::Processes,
        Self::Cpu,
        Self::Ram,
        Self::Memory,
        Self::Storage,
        Self::Registers,
        Self::Cache,
        Self::Simulation,
    ];
    pub(crate) fn title(self) -> &'static str {
        match self {
            Self::Overview => "Обзор",
            Self::Processes => "Процессы",
            Self::Cpu => "CPU",
            Self::Ram => "RAM",
            Self::Memory => "Память",
            Self::Storage => "Накопители",
            Self::Registers => "Регистры",
            Self::Cache => "Кэш",
            Self::Simulation => "Симуляция",
        }
    }
    pub(crate) fn description(self) -> &'static str {
        match self {
            Self::Overview => "Система и доступность разделов",
            Self::Processes => "Список процессов/потоков и цель инспекции",
            Self::Cpu => "Топология CPU и живая нагрузка",
            Self::Ram => "Статистика оперативной памяти",
            Self::Memory => "Виртуальные регионы и HEX выбранной цели",
            Self::Storage => "Тома, живой Disk I/O и HEX файла",
            Self::Registers => "Регистры выбранного потока",
            Self::Cache => "Характеристики и иерархия кэшей",
            Self::Simulation => "Отдельные модели иерархии памяти",
        }
    }
    pub(crate) fn pending(self) -> &'static str {
        match self {
            Self::Overview => "",
            Self::Processes => {
                "Панель процессов подключена: MEASURED enumeration и выбор цели инспекции."
            }
            Self::Cpu => "Панель CPU подключена: топология при запуске и живая нагрузка.",
            Self::Ram => "Панель RAM подключена: снимки, интервал и история использования.",
            Self::Memory => {
                "Панель памяти подключена: карта виртуальных регионов текущего процесса."
            }
            Self::Storage => {
                "Панель накопителей подключена: тома, живой Disk I/O и read-only HEX файла."
            }
            Self::Registers => {
                "Панель регистров подключена: снимок текущего потока (другие потоки — отдельно)."
            }
            Self::Cache => {
                "Панель кэша подключена: иерархия L1/L2/L3 и sharing (без содержимого линий)."
            }
            Self::Simulation => {
                "Панель симуляции подключена: учебная модель L1/L2/RAM с пометкой SIMULATED."
            }
        }
    }
}

pub(crate) struct MonitorApp {
    page: Page,
    dark: bool,
    identity: PlatformResult<SystemIdentity>,
    capabilities: Vec<(PlatformFeature, Availability)>,
    cpu: PlatformResult<CpuInfo>,
    storage: PlatformResult<Vec<StorageVolume>>,
    storage_panel: StoragePanel,
    live: LiveTelemetry,
    processes: ProcessesPanel,
    memory: MemoryPanel,
    registers: RegistersPanel,
    cache: PlatformResult<CacheTopology>,
    cache_counters: PlatformResult<CacheCounters>,
    simulation: SimulationPanel,
    read_memory: Box<dyn Fn(MemoryReadRequest) -> PlatformResult<MemoryBytes>>,
    refresh_registers: Box<dyn Fn() -> PlatformResult<RegisterSnapshot>>,
    thread_registers: Box<dyn Fn(u64, u64) -> PlatformResult<RegisterSnapshot>>,
    refresh_processes: Box<dyn Fn() -> PlatformResult<Vec<ProcessInfo>>>,
    list_threads: Box<dyn Fn(u64) -> PlatformResult<Vec<ThreadInfo>>>,
    refresh_cache_counters: Box<dyn Fn() -> PlatformResult<CacheCounters>>,
    read_file: Box<dyn Fn(FileReadRequest) -> PlatformResult<FileBytes>>,
}

impl MonitorApp {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        cc: &eframe::CreationContext<'_>,
        identity: PlatformResult<SystemIdentity>,
        capabilities: Vec<(PlatformFeature, Availability)>,
        cpu: PlatformResult<CpuInfo>,
        storage: PlatformResult<Vec<StorageVolume>>,
        live: LiveTelemetry,
        regions: PlatformResult<Vec<VirtualMemoryRegion>>,
        registers: PlatformResult<RegisterSnapshot>,
        cache: PlatformResult<CacheTopology>,
        cache_counters: PlatformResult<CacheCounters>,
        processes: PlatformResult<Vec<ProcessInfo>>,
        target: InspectionTarget,
        read_memory: Box<dyn Fn(MemoryReadRequest) -> PlatformResult<MemoryBytes>>,
        refresh_registers: Box<dyn Fn() -> PlatformResult<RegisterSnapshot>>,
        thread_registers: Box<dyn Fn(u64, u64) -> PlatformResult<RegisterSnapshot>>,
        refresh_processes: Box<dyn Fn() -> PlatformResult<Vec<ProcessInfo>>>,
        list_threads: Box<dyn Fn(u64) -> PlatformResult<Vec<ThreadInfo>>>,
        refresh_cache_counters: Box<dyn Fn() -> PlatformResult<CacheCounters>>,
        read_file: Box<dyn Fn(FileReadRequest) -> PlatformResult<FileBytes>>,
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
            capabilities,
            cpu,
            storage,
            storage_panel: StoragePanel::default(),
            live,
            processes: ProcessesPanel::new(processes, target),
            memory: MemoryPanel::new(regions),
            registers: RegistersPanel::new(registers),
            cache,
            cache_counters,
            simulation: SimulationPanel::new(),
            read_memory,
            refresh_registers,
            thread_registers,
            refresh_processes,
            list_threads,
            refresh_cache_counters,
            read_file,
        }
    }

    // No OS queries, worker startup or blocking receives inside a UI frame.
    // Memory HEX reads run only on explicit user button clicks.
    fn render(&mut self, ctx: &egui::Context) {
        self.live.poll(ctx);
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
                ui.label(egui::RichText::new("Processes → Quality").small().weak());
            });
        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.add_space(16.0);
                ui.heading(self.page.title());
                ui.label(egui::RichText::new(self.page.description()).weak());
                ui.add_space(20.0);
                match self.page {
                    Page::Overview => panels::overview(ui, &self.identity, &self.capabilities),
                    Page::Processes => {
                        let refresh = self.refresh_processes.as_ref();
                        let threads = self.list_threads.as_ref();
                        self.processes.show(ui, refresh, threads);
                    }
                    Page::Cpu => cpu::show(ui, &self.cpu, &mut self.live),
                    Page::Storage => {
                        let read_file = self.read_file.as_ref();
                        self.storage_panel
                            .show(ui, &self.storage, &mut self.live, read_file);
                    }
                    Page::Ram => self.live.show_ram(ui),
                    Page::Memory => {
                        if self.processes.target().is_current_process() {
                            let read_memory = self.read_memory.as_ref();
                            self.memory.show(ui, read_memory);
                        } else {
                            ui.monospace("NOT_IMPLEMENTED");
                            ui.label(
                                "Чтение памяти чужого процесса ещё не подключено. Debug/open-process API появится позже; SIMULATED данные сюда не подставляются.",
                            );
                        }
                    }
                    Page::Registers => {
                        let target = self.processes.target().clone();
                        let other_thread = target.thread.is_some();
                        let pid = target.process.pid;
                        let tid = target.thread.as_ref().map(|thread| thread.tid);
                        let refresh_current = self.refresh_registers.as_ref();
                        let refresh_thread = self.thread_registers.as_ref();
                        self.registers.show(ui, other_thread, &|| {
                            if let Some(tid) = tid {
                                refresh_thread(pid, tid)
                            } else {
                                refresh_current()
                            }
                        });
                    }
                    Page::Cache => {
                        let refresh = self.refresh_cache_counters.as_ref();
                        cache::show(ui, &self.cache, &mut self.cache_counters, refresh);
                    }
                    Page::Simulation => self.simulation.show(ui),
                }
            });
        });
    }
}

impl eframe::App for MonitorApp {
    fn on_exit(&mut self, _: Option<&eframe::glow::Context>) {
        if let Err(error) = self.live.shutdown() {
            eprintln!("Telemetry shutdown failed: {error}");
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

    fn fixture_app(page: Page, dark: bool, identity: PlatformResult<SystemIdentity>) -> MonitorApp {
        MonitorApp {
            page,
            dark,
            identity,
            capabilities: PlatformFeature::ALL
                .into_iter()
                .map(|feature| (feature, Availability::NotImplemented))
                .collect(),
            cpu: PlatformResult::NotImplemented("test fixture".into()),
            storage: PlatformResult::NotImplemented("test fixture".into()),
            storage_panel: StoragePanel::default(),
            live: LiveTelemetry::new(Err(DiagnosticError::PlatformError("test".into()))),
            processes: ProcessesPanel::new(
                PlatformResult::NotImplemented("test fixture".into()),
                InspectionTarget::current_process(0, "u-monitor"),
            ),
            memory: MemoryPanel::new(PlatformResult::NotImplemented("test fixture".into())),
            registers: RegistersPanel::new(PlatformResult::NotImplemented("test fixture".into())),
            cache: PlatformResult::NotImplemented("test fixture".into()),
            cache_counters: PlatformResult::NotImplemented("test fixture".into()),
            simulation: SimulationPanel::new(),
            read_memory: Box::new(|_| PlatformResult::NotImplemented("test fixture".into())),
            refresh_registers: Box::new(|| PlatformResult::NotImplemented("test fixture".into())),
            thread_registers: Box::new(|_, _| {
                PlatformResult::NotImplemented("test fixture".into())
            }),
            refresh_processes: Box::new(|| PlatformResult::NotImplemented("test fixture".into())),
            list_threads: Box::new(|_| PlatformResult::NotImplemented("test fixture".into())),
            refresh_cache_counters: Box::new(|| {
                PlatformResult::NotImplemented("test fixture".into())
            }),
            read_file: Box::new(|_| PlatformResult::NotImplemented("test fixture".into())),
        }
    }

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
                    let mut app = fixture_app(page, dark, identity.clone());
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
            let mut app = fixture_app(
                Page::Overview,
                dark,
                PlatformResult::NotImplemented("test fixture".into()),
            );
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

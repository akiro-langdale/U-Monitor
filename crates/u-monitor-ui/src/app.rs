use crate::{
    cache, cpu, explorer::ExplorerState, live::LiveTelemetry, memory::MemoryPanel, panels,
    processes::ProcessesPanel, registers::RegistersPanel, simulation::SimulationPanel,
    storage::StoragePanel,
};

const THEME_KEY: &str = "u-monitor.ui.theme";

#[cfg(windows)]
struct ForeignMemory {
    pid: u64,
    opened: Result<(ProcessMemory, MemoryPanel), String>,
}

#[cfg(windows)]
fn show_foreign_memory(ui: &mut egui::Ui, pid: u64, memory: &mut Option<ForeignMemory>) {
    if let Some(ForeignMemory {
        opened: Ok((session, panel)),
        ..
    }) = memory
    {
        if ui.button("Обновить карту памяти").clicked() {
            *panel = MemoryPanel::new(session.regions());
        }
        panel.show(ui, &|request| session.read(request));
        return;
    }
    if ui.button("Открыть карту памяти").clicked() {
        let opened = match ProcessMemory::open(pid) {
            PlatformResult::Success(session) => {
                let session = session.value;
                let panel = MemoryPanel::new(session.regions());
                Ok((session, panel))
            }
            PlatformResult::PermissionRequired(reason) => Err(format!("Доступ запрещён. {reason}")),
            PlatformResult::Unavailable(reason) | PlatformResult::NotImplemented(reason) => {
                Err(reason)
            }
            PlatformResult::Error(error) => Err(error.to_string()),
        };
        *memory = Some(ForeignMemory { pid, opened });
        ui.ctx().request_repaint();
    }
    if let Some(ForeignMemory {
        opened: Err(reason),
        ..
    }) = memory
    {
        ui.colored_label(ui.visuals().error_fg_color, reason);
    }
}

fn configure_style(ctx: &egui::Context) {
    for (theme, dark) in [(egui::Theme::Dark, true), (egui::Theme::Light, false)] {
        ctx.set_visuals_of(theme, crate::design::visuals(dark));
        ctx.style_mut_of(theme, |style| {
            style.spacing.item_spacing = egui::vec2(10.0, 7.0);
            style.spacing.button_padding = egui::vec2(10.0, 5.0);
            style.spacing.interact_size.y = 26.0;
            style.spacing.indent = 18.0;
        });
    }
}

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
#[cfg(windows)]
use u_monitor_platform::ProcessMemory;

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
    Gpu,
    Simulation,
}

impl Page {
    pub(crate) const ALL: [Self; 10] = [
        Self::Overview,
        Self::Processes,
        Self::Cpu,
        Self::Ram,
        Self::Memory,
        Self::Storage,
        Self::Registers,
        Self::Cache,
        Self::Gpu,
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
            Self::Gpu => "GPU",
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
            Self::Gpu => "Адаптеры и заявленная VRAM через U-Library",
            Self::Simulation => "Отдельные модели иерархии памяти",
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
    #[cfg(windows)]
    foreign_memory: Option<ForeignMemory>,
    registers: RegistersPanel,
    cache: PlatformResult<CacheTopology>,
    cache_counters: PlatformResult<CacheCounters>,
    gpu: crate::gpu::GpuPanel,
    simulation: SimulationPanel,
    read_memory: Box<dyn Fn(MemoryReadRequest) -> PlatformResult<MemoryBytes>>,
    refresh_registers: Box<dyn Fn() -> PlatformResult<RegisterSnapshot>>,
    thread_registers: Box<dyn Fn(u64, u64) -> PlatformResult<RegisterSnapshot>>,
    refresh_processes: Box<dyn Fn() -> PlatformResult<Vec<ProcessInfo>>>,
    list_threads: Box<dyn Fn(u64) -> PlatformResult<Vec<ThreadInfo>>>,
    refresh_cache_counters: Box<dyn Fn() -> PlatformResult<CacheCounters>>,
    read_file: Box<dyn Fn(FileReadRequest) -> PlatformResult<FileBytes>>,
    explorer: ExplorerState,
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
        configure_style(&cc.egui_ctx);
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
            #[cfg(windows)]
            foreign_memory: None,
            registers: RegistersPanel::new(registers),
            simulation: SimulationPanel::new(&cache),
            cache,
            cache_counters,
            gpu: crate::gpu::GpuPanel::load(),
            read_memory,
            refresh_registers,
            thread_registers,
            refresh_processes,
            list_threads,
            refresh_cache_counters,
            read_file,
            explorer: ExplorerState::default(),
        }
    }

    // No OS queries, worker startup or blocking receives inside a UI frame.
    // Memory HEX reads run only on explicit user button clicks.
    fn render(&mut self, ctx: &egui::Context) {
        self.live.poll(ctx);
        #[cfg(windows)]
        if self.foreign_memory.as_ref().is_some_and(|memory| {
            self.processes.target().is_current_process()
                || memory.pid != self.processes.target().process.pid
        }) {
            self.foreign_memory = None;
        }
        egui::TopBottomPanel::top("header")
            .frame(
                egui::Frame::new()
                    .fill(ctx.style().visuals.window_fill)
                    .inner_margin(egui::Margin::symmetric(18, 10)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("U-Monitor")
                            .size(21.0)
                            .strong()
                            .color(ui.visuals().hyperlink_color),
                    );
                    ui.weak(env!("CARGO_PKG_VERSION"));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let next = if self.dark {
                            "Светлая тема"
                        } else {
                            "Тёмная тема"
                        };
                        if ui.button(next).clicked() {
                            self.dark = !self.dark;
                            ctx.set_theme(if self.dark {
                                egui::Theme::Dark
                            } else {
                                egui::Theme::Light
                            });
                        }
                    });
                });
            });
        egui::SidePanel::left("navigation")
            .exact_width(if ctx.content_rect().width() < 850.0 {
                128.0
            } else {
                170.0
            })
            .resizable(false)
            .frame(
                egui::Frame::new()
                    .fill(ctx.style().visuals.window_fill)
                    .inner_margin(10),
            )
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for (label, pages) in [
                        ("СИСТЕМА", &Page::ALL[..4]),
                        ("ИНСПЕКЦИЯ", &Page::ALL[4..9]),
                        ("МОДЕЛИ", &Page::ALL[9..]),
                    ] {
                        ui.add_space(8.0);
                        ui.label(egui::RichText::new(label).size(10.0).weak().strong());
                        for &page in pages {
                            let selected = self.page == page;
                            let response = ui.add_sized(
                                [ui.available_width(), 27.0],
                                egui::Button::new(page.title()).selected(selected),
                            );
                            if response.clicked() {
                                self.page = page;
                            }
                        }
                    }
                });
            });
        let page = self.page;
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(ctx.style().visuals.panel_fill).inner_margin(18))
            .show(ctx, |ui| {
            egui::ScrollArea::vertical().id_salt(page.title()).auto_shrink(false).show(ui, |ui| {
                ui.label(egui::RichText::new(page.title()).size(32.0).strong());
                ui.weak(page.description());
                if matches!(page, Page::Memory | Page::Registers) {
                    let target = self.processes.target();
                    ui.horizontal_wrapped(|ui| {
                        ui.weak("Цель");
                        ui.monospace(format!("{} · PID {}", target.process.name, target.process.pid));
                        if let Some(thread) = &target.thread {
                            ui.monospace(format!("TID {}", thread.tid));
                        }
                        if ui.link("Выбрать процесс").clicked() {
                            self.page = Page::Processes;
                        }
                    });
                }
                ui.add_space(12.0);
                let frame = if page == Page::Overview {
                    egui::Frame::new()
                } else {
                    crate::design::surface(ui)
                };
                frame.show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                match page {
                    Page::Overview => {
                        if let Some(next) = crate::dashboard::show(
                            ui,
                            &self.live,
                            &self.cache,
                            &self.storage,
                            &self.registers.snapshot,
                        ) {
                            self.page = next;
                        }
                        ui.add_space(16.0);
                        panels::overview(ui, &self.identity, &self.capabilities);
                        ui.add_space(16.0);
                        crate::design::section(ui, "Экспорт диагностики");
                        ui.label(
                            "JSON со статусами панелей и текущим cross-panel selection (без содержимого памяти).",
                        );
                        if ui.button("Экспорт JSON snapshot").clicked() {
                            let json = crate::explorer::build_snapshot_json(
                                &self.identity,
                                &self.capabilities,
                                &self.cpu,
                                &self.storage,
                                &self.cache,
                                &self.registers.snapshot,
                                &self.explorer.selection,
                            );
                            match crate::explorer::export_snapshot_to_temp(&json) {
                                Ok(path) => {
                                    self.explorer.last_export_path = Some(path);
                                    self.explorer.last_export_error = None;
                                }
                                Err(error) => {
                                    self.explorer.last_export_error = Some(error);
                                    self.explorer.last_export_path = None;
                                }
                            }
                        }
                        if let Some(path) = &self.explorer.last_export_path {
                            ui.monospace(format!("Saved: {}", path.display()));
                        }
                        if let Some(error) = &self.explorer.last_export_error {
                            ui.colored_label(ui.visuals().error_fg_color, error);
                        }
                        ui.label(format!(
                            "Selection: cpu={:?} cache_node={:?} sim_addr={:?}",
                            self.explorer.selection.logical_cpu,
                            self.explorer.selection.cache_node,
                            self.explorer
                                .selection
                                .sim_address
                                .map(|addr| format!("{addr:#x}"))
                        ));
                    }
                    Page::Processes => {
                        let refresh = self.refresh_processes.as_ref();
                        let threads = self.list_threads.as_ref();
                        self.processes.show(ui, refresh, threads);
                    }
                    Page::Cpu => {
                        let selected = self.explorer.selection.logical_cpu;
                        if let Some(index) = cpu::show(ui, &self.cpu, &mut self.live, selected) {
                            self.explorer.select_logical_cpu(index, &self.cache);
                            self.page = Page::Cache;
                        }
                    }
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
                            #[cfg(windows)]
                            show_foreign_memory(
                                ui,
                                self.processes.target().process.pid,
                                &mut self.foreign_memory,
                            );
                            #[cfg(not(windows))]
                            ui.label("Чтение памяти выбранного процесса доступно на Windows.");
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
                        cache::show(
                            ui,
                            &self.cache,
                            &mut self.cache_counters,
                            refresh,
                            self.explorer.selection.logical_cpu,
                            self.explorer.selection.cache_node,
                        );
                        if ui
                            .add_enabled(
                                self.explorer.selection.sim_address.is_some(),
                                egui::Button::new("Открыть Simulation с адресом линии"),
                            )
                            .clicked()
                            && let Some(address) = self.explorer.selection.sim_address
                        {
                            self.simulation.set_address(address);
                            self.page = Page::Simulation;
                        }
                    }
                    Page::Gpu => self.gpu.show(ui),
                    Page::Simulation => self.simulation.show(ui),
                }
                });
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
            #[cfg(windows)]
            foreign_memory: None,
            registers: RegistersPanel::new(PlatformResult::NotImplemented("test fixture".into())),
            cache: PlatformResult::NotImplemented("test fixture".into()),
            cache_counters: PlatformResult::NotImplemented("test fixture".into()),
            gpu: crate::gpu::GpuPanel::load(),
            simulation: SimulationPanel::new(&PlatformResult::NotImplemented(
                "test fixture".into(),
            )),
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
            explorer: ExplorerState::default(),
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
                    configure_style(&ctx);
                    ctx.set_theme(if dark {
                        egui::Theme::Dark
                    } else {
                        egui::Theme::Light
                    });
                    let mut app = fixture_app(page, dark, identity.clone());
                    for size in [[1040.0, 700.0], [720.0, 480.0]] {
                        let input = egui::RawInput {
                            screen_rect: Some(egui::Rect::from_min_size(
                                egui::Pos2::ZERO,
                                size.into(),
                            )),
                            ..Default::default()
                        };
                        let output = ctx.run(input, |ctx| app.render(ctx));
                        assert!(!output.shapes.is_empty(), "{page:?} at {size:?}");
                    }
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

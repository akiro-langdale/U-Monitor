mod app;
mod cpu;
mod live;
mod memory;
mod panels;
mod ram;
mod registers;
mod storage;

use app::MonitorApp;
use eframe::egui;
use live::LiveTelemetry;
use std::time::Duration;
use u_monitor_core::PlatformAdapter;
use u_monitor_platform::{NativePlatformAdapter, NativeTelemetryAdapter};
use u_monitor_telemetry::{TelemetryWorker, WorkerConfig};

fn main() -> eframe::Result {
    // Composition root: acquire immutable identity once, before rendering frames.
    let identity = NativePlatformAdapter.system_identity();
    let cpu = NativePlatformAdapter.cpu_info();
    let storage = NativePlatformAdapter.storage_volumes();
    let regions = NativePlatformAdapter.virtual_memory_regions();
    let registers = NativePlatformAdapter.current_thread_registers();
    let live = LiveTelemetry::new(TelemetryWorker::start(
        NativeTelemetryAdapter::default(),
        WorkerConfig {
            interval: Duration::from_secs(1),
            history_capacity: 60,
            channel_capacity: 4,
        },
    ));
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_app_id("u-monitor")
            .with_inner_size([1040.0, 700.0])
            .with_min_inner_size([720.0, 480.0]),
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };
    eframe::run_native(
        concat!("U-Monitor ", env!("CARGO_PKG_VERSION")),
        options,
        Box::new(move |cc| {
            Ok(Box::new(MonitorApp::new(
                cc,
                identity,
                cpu,
                storage,
                live,
                regions,
                registers,
                Box::new(|request| NativePlatformAdapter.read_current_process_bytes(request)),
                Box::new(|| NativePlatformAdapter.current_thread_registers()),
            )))
        }),
    )
}

//! Process-wide tracing configured from `U_MONITOR_LOG` and `RUST_LOG`.

use std::{fs::OpenOptions, io, sync::OnceLock};
use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};
use u_monitor_platform::scrub_path_for_log;

static INIT: OnceLock<()> = OnceLock::new();

/// Installs a global tracing subscriber once per process.
pub fn init_tracing() {
    let _ = INIT.get_or_init(|| {
        let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
            EnvFilter::new("info,u_monitor=info,u_monitor_platform=debug,u_monitor_telemetry=debug")
        });
        if let Ok(path) = std::env::var("U_MONITOR_LOG") {
            let scrubbed = scrub_path_for_log(&path);
            match OpenOptions::new().create(true).append(true).open(&path) {
                Ok(file) => {
                    tracing_subscriber::registry()
                        .with(filter)
                        .with(fmt::layer().with_writer(file).with_ansi(false))
                        .init();
                    tracing::info!(log_target = %scrubbed, "tracing file sink enabled");
                    return;
                }
                Err(error) => {
                    tracing_subscriber::registry()
                        .with(filter)
                        .with(fmt::layer().with_writer(io::stderr))
                        .init();
                    tracing::warn!(
                        log_target = %scrubbed,
                        error = %error,
                        "failed to open U_MONITOR_LOG; using stderr"
                    );
                    return;
                }
            }
        }
        tracing_subscriber::registry()
            .with(filter)
            .with(fmt::layer().with_writer(io::stderr))
            .init();
        tracing::debug!("tracing initialized (stderr)");
    });
}

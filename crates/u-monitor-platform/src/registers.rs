//! Register capture via U-Library.

use std::time::SystemTime;
use u_library_registers::{self, RegisterError};
use u_monitor_core::{
    DataSource, Measurement, PlatformResult, ProcessId, RegisterArchitecture, RegisterSnapshot,
    RegisterValue, ThreadId,
};

fn measured<T>(value: T) -> PlatformResult<T> {
    PlatformResult::Success(Measurement {
        value,
        source: DataSource::Measured,
        timestamp: SystemTime::now().into(),
    })
}

fn adapt(snapshot: u_library_registers::RegisterSnapshot) -> RegisterSnapshot {
    RegisterSnapshot {
        architecture: match snapshot.architecture {
            u_library_registers::RegisterArchitecture::X86_64 => RegisterArchitecture::X86_64,
            u_library_registers::RegisterArchitecture::AArch64 => RegisterArchitecture::AArch64,
        },
        thread_id: measured(snapshot.thread_id),
        registers: snapshot
            .registers
            .into_iter()
            .map(|reg| RegisterValue::new(reg.name, reg.bits, reg.width_bits))
            .collect(),
    }
}

fn map_error(error: RegisterError) -> PlatformResult<RegisterSnapshot> {
    match error {
        RegisterError::Unsupported(message) => PlatformResult::NotImplemented(message),
        RegisterError::PermissionRequired(message) => PlatformResult::PermissionRequired(message),
        RegisterError::Unavailable(message) => PlatformResult::Unavailable(message),
    }
}

pub(crate) fn capture_current_thread() -> PlatformResult<RegisterSnapshot> {
    match u_library_registers::capture_current_thread() {
        Ok(snapshot) => measured(adapt(snapshot)),
        Err(error) => map_error(error),
    }
}

/// Captures registers of another thread. Caller must obtain explicit UI consent first.
pub(crate) fn capture_thread(
    process_id: ProcessId,
    thread_id: ThreadId,
) -> PlatformResult<RegisterSnapshot> {
    match u_library_registers::capture_thread(process_id, thread_id) {
        Ok(snapshot) => measured(adapt(snapshot)),
        Err(error) => map_error(error),
    }
}

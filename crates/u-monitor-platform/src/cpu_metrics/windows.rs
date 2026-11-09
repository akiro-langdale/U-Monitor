//! Windows CPU utilization via kernel counters, not PDH.
//!
//! `sysinfo` on Windows reads `\Processor(*)\% Idle Time`. When those English
//! PDH counters are unavailable or never update, usage stays at the initial 0
//! and would be mislabelled as a real measurement. `GetSystemTimes` and
//! `NtQuerySystemInformation(SystemProcessorPerformanceInformation)` use the
//! same counter family as Task Manager and remain valid without PDH.

use super::{CpuSamplerState, LogicalSample, MINIMUM_INTERVAL, from_report, mhz, percent};
use std::{
    mem::{MaybeUninit, size_of},
    time::{Instant, SystemTime},
};
use u_monitor_core::{CpuMetrics, DiagnosticError, PlatformResult};
use windows::Wdk::System::SystemInformation::{
    NtQuerySystemInformation, SystemProcessorPerformanceInformation,
};
use windows::Win32::Foundation::NTSTATUS;
use windows::Win32::System::Power::{
    CallNtPowerInformation, PROCESSOR_POWER_INFORMATION, ProcessorInformation,
};
use windows::Win32::System::SystemInformation::{GetSystemInfo, SYSTEM_INFO};
use windows::Win32::System::Threading::GetSystemTimes;

/// Matches the documented SYSTEM_PROCESSOR_PERFORMANCE_INFORMATION layout.
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct ProcessorPerformance {
    idle_time: i64,
    kernel_time: i64,
    user_time: i64,
    dpc_time: i64,
    interrupt_time: i64,
    interrupt_count: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct TickTriple {
    idle: u64,
    /// Kernel time includes idle time on Windows.
    kernel: u64,
    user: u64,
}

impl TickTriple {
    fn from_filetimes(
        idle: windows::Win32::Foundation::FILETIME,
        kernel: windows::Win32::Foundation::FILETIME,
        user: windows::Win32::Foundation::FILETIME,
    ) -> Self {
        Self {
            idle: filetime_to_u64(idle),
            kernel: filetime_to_u64(kernel),
            user: filetime_to_u64(user),
        }
    }

    fn from_processor(info: &ProcessorPerformance) -> Self {
        Self {
            idle: info.idle_time as u64,
            kernel: info.kernel_time as u64,
            user: info.user_time as u64,
        }
    }

    /// Busy percent for the interval ending at `next`. `None` if no time elapsed.
    fn usage_toward(self, next: Self) -> Option<f32> {
        let idle = next.idle.saturating_sub(self.idle);
        let kernel = next.kernel.saturating_sub(self.kernel);
        let user = next.user.saturating_sub(self.user);
        let total = kernel.saturating_add(user);
        if total == 0 {
            return None;
        }
        let busy = total.saturating_sub(idle);
        Some((busy as f64 * 100.0 / total as f64) as f32)
    }
}

fn filetime_to_u64(value: windows::Win32::Foundation::FILETIME) -> u64 {
    ((value.dwHighDateTime as u64) << 32) | value.dwLowDateTime as u64
}

fn read_total() -> Result<TickTriple, String> {
    // SAFETY: GetSystemTimes writes three distinct FILETIME outputs we own.
    unsafe {
        let mut idle = MaybeUninit::uninit();
        let mut kernel = MaybeUninit::uninit();
        let mut user = MaybeUninit::uninit();
        GetSystemTimes(
            Some(idle.as_mut_ptr()),
            Some(kernel.as_mut_ptr()),
            Some(user.as_mut_ptr()),
        )
        .map_err(|error| format!("GetSystemTimes failed: {error}"))?;
        Ok(TickTriple::from_filetimes(
            idle.assume_init(),
            kernel.assume_init(),
            user.assume_init(),
        ))
    }
}

fn logical_processor_count() -> usize {
    // SAFETY: GetSystemInfo only writes into the provided SYSTEM_INFO.
    unsafe {
        let mut info = SYSTEM_INFO::default();
        GetSystemInfo(&mut info);
        info.dwNumberOfProcessors.max(1) as usize
    }
}

fn read_per_processor() -> Result<Vec<TickTriple>, String> {
    let count = logical_processor_count();
    let mut buffer = vec![ProcessorPerformance::default(); count];
    let bytes = (size_of::<ProcessorPerformance>() * count) as u32;
    let mut returned = 0u32;
    // SAFETY: buffer length matches the requested NT query size; entries are POD.
    let status = unsafe {
        NtQuerySystemInformation(
            SystemProcessorPerformanceInformation,
            buffer.as_mut_ptr().cast(),
            bytes,
            &mut returned,
        )
    };
    if status != NTSTATUS(0) {
        return Err(format!(
            "NtQuerySystemInformation(SystemProcessorPerformanceInformation) failed: {status:?}"
        ));
    }
    let available = (returned as usize) / size_of::<ProcessorPerformance>();
    if available == 0 {
        return Err("OS returned no logical processor counters".into());
    }
    buffer.truncate(available);
    Ok(buffer.iter().map(TickTriple::from_processor).collect())
}

#[derive(Clone, Copy)]
struct FrequencyPair {
    current_mhz: u64,
    max_mhz: u64,
}

fn read_frequencies(count: usize) -> Result<Vec<FrequencyPair>, String> {
    let mut infos = vec![PROCESSOR_POWER_INFORMATION::default(); count];
    let bytes = (size_of::<PROCESSOR_POWER_INFORMATION>() * count) as u32;
    // SAFETY: output buffer matches CallNtPowerInformation size for ProcessorInformation.
    let status = unsafe {
        CallNtPowerInformation(
            ProcessorInformation,
            None,
            0,
            Some(infos.as_mut_ptr().cast()),
            bytes,
        )
    };
    if status != NTSTATUS(0) {
        return Err(format!(
            "CallNtPowerInformation(ProcessorInformation) failed: {status:?}"
        ));
    }
    Ok(infos
        .into_iter()
        .map(|info| FrequencyPair {
            current_mhz: u64::from(info.CurrentMhz),
            max_mhz: u64::from(info.MaxMhz),
        })
        .collect())
}

#[derive(Default)]
pub(super) struct WindowsCpuBackend {
    last_refresh: Option<Instant>,
    previous_total: Option<TickTriple>,
    previous_processors: Option<Vec<TickTriple>>,
}

impl CpuSamplerState for WindowsCpuBackend {
    fn sample_at(&mut self, now: Instant) -> PlatformResult<CpuMetrics> {
        if self
            .last_refresh
            .is_some_and(|last| now.duration_since(last) < MINIMUM_INTERVAL)
        {
            return PlatformResult::Unavailable(
                "CPU sampling interval is shorter than the backend minimum".into(),
            );
        }

        let total = match read_total() {
            Ok(value) => value,
            Err(error) => {
                return PlatformResult::Error(DiagnosticError::PlatformError(error));
            }
        };
        let processors = match read_per_processor() {
            Ok(value) => value,
            Err(error) => {
                return PlatformResult::Error(DiagnosticError::PlatformError(error));
            }
        };
        if processors.is_empty() {
            return PlatformResult::Unavailable(
                "No logical processors were reported by the OS".into(),
            );
        }

        let completed = Instant::now();
        let previous_total = self.previous_total.replace(total);
        let previous_processors = self.previous_processors.replace(processors.clone());
        let previous_instant = self.last_refresh.replace(completed);

        let (Some(previous_total), Some(previous_processors), Some(previous_instant)) =
            (previous_total, previous_processors, previous_instant)
        else {
            return PlatformResult::Unavailable(
                "CPU counters initialized; waiting for the next measurement".into(),
            );
        };

        let timestamp = SystemTime::now().into();
        let Some(total_usage) = previous_total.usage_toward(total) else {
            return PlatformResult::Unavailable(
                "CPU counters did not advance between samples".into(),
            );
        };

        let frequencies = read_frequencies(processors.len());
        let frequency_error = frequencies.as_ref().err().cloned();

        let logical = previous_processors
            .iter()
            .zip(processors.iter())
            .enumerate()
            .map(|(index, (prev, next))| {
                let name = format!("CPU {}", index + 1);
                let usage = match prev.usage_toward(*next) {
                    Some(value) => percent(value, timestamp),
                    None => PlatformResult::Unavailable(
                        "logical processor counters did not advance".into(),
                    ),
                };
                let (current_mhz, max_mhz) = match (&frequencies, frequency_error.as_ref()) {
                    (Ok(values), _) => match values.get(index).copied() {
                        Some(value) => (
                            mhz(value.current_mhz, timestamp),
                            mhz(value.max_mhz, timestamp),
                        ),
                        None => (
                            PlatformResult::Unavailable(
                                "CPU frequency entry missing for logical processor".into(),
                            ),
                            PlatformResult::Unavailable(
                                "CPU frequency entry missing for logical processor".into(),
                            ),
                        ),
                    },
                    (Err(_), Some(error)) => (
                        PlatformResult::Unavailable(error.clone()),
                        PlatformResult::Unavailable(error.clone()),
                    ),
                    (Err(_), None) => (
                        PlatformResult::Unavailable("CPU frequency unavailable".into()),
                        PlatformResult::Unavailable("CPU frequency unavailable".into()),
                    ),
                };
                LogicalSample {
                    name,
                    usage,
                    current_mhz,
                    max_mhz,
                }
            });

        from_report(
            total_usage,
            logical,
            completed.duration_since(previous_instant),
            timestamp,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usage_rejects_empty_windows_and_preserves_idle_and_busy() {
        let base = TickTriple {
            idle: 100,
            kernel: 100,
            user: 0,
        };
        assert_eq!(base.usage_toward(base), None);
        assert_eq!(
            base.usage_toward(TickTriple {
                idle: 200,
                kernel: 200,
                user: 0,
            }),
            Some(0.0)
        );
        assert_eq!(
            base.usage_toward(TickTriple {
                idle: 100,
                kernel: 150,
                user: 50,
            }),
            Some(100.0)
        );
        let half = base.usage_toward(TickTriple {
            idle: 150,
            kernel: 175,
            user: 25,
        });
        assert!(half.is_some_and(|value| (value - 50.0).abs() < f32::EPSILON));
    }
}

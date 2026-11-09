//! Linux `perf_event_open` sampling for hardware cache references/misses.

use std::{
    io, mem,
    os::fd::{FromRawFd, IntoRawFd, OwnedFd},
    time::{Duration, Instant, SystemTime},
};
use u_monitor_core::{CacheCounters, DataSource, DiagnosticError, Measurement, PlatformResult};

const PERF_TYPE_HARDWARE: u32 = 0;
const PERF_COUNT_HW_CACHE_REFERENCES: u64 = 2;
const PERF_COUNT_HW_CACHE_MISSES: u64 = 3;
const SAMPLE_WINDOW: Duration = Duration::from_millis(100);

// PERF_EVENT_IOC_* = _IO('$', n) on Linux.
const PERF_EVENT_IOC_ENABLE: libc::c_ulong = 0x2400;
const PERF_EVENT_IOC_DISABLE: libc::c_ulong = 0x2401;
const PERF_EVENT_IOC_RESET: libc::c_ulong = 0x2402;

#[repr(C)]
struct PerfEventAttr {
    type_: u32,
    size: u32,
    config: u64,
    sample_period: u64,
    sample_type: u64,
    read_format: u64,
    flags: u64,
    wakeup_events: u32,
    bp_type: u32,
    bp_addr: u64,
    bp_len: u64,
    branch_sample_type: u64,
    sample_regs_user: u64,
    sample_stack_user: u32,
    clockid: i32,
    sample_regs_intr: u64,
    aux_watermark: u32,
    sample_max_stack: u16,
    __reserved_2: u16,
    aux_sample_size: u32,
    __reserved_3: u32,
}

impl Default for PerfEventAttr {
    fn default() -> Self {
        // SAFETY: zeroed attr is the documented starting point for perf_event_open.
        unsafe { mem::zeroed() }
    }
}

fn measured<T>(value: T) -> PlatformResult<T> {
    PlatformResult::Success(Measurement {
        value,
        source: DataSource::Measured,
        timestamp: SystemTime::now().into(),
    })
}

pub(super) fn sample() -> PlatformResult<CacheCounters> {
    let references = open_counter(PERF_COUNT_HW_CACHE_REFERENCES);
    let misses = open_counter(PERF_COUNT_HW_CACHE_MISSES);

    match (&references, &misses) {
        (Err(OpenError::Permission(msg)), _) | (_, Err(OpenError::Permission(msg))) => {
            return PlatformResult::PermissionRequired(msg.clone());
        }
        (Err(OpenError::Unavailable(_)), Err(OpenError::Unavailable(msg))) => {
            return PlatformResult::Unavailable(msg.clone());
        }
        _ => {}
    }

    let refs_fd = references.as_ref().ok().map(|fd| fd.as_raw_borrow());
    let miss_fd = misses.as_ref().ok().map(|fd| fd.as_raw_borrow());

    for fd in [refs_fd, miss_fd].into_iter().flatten() {
        reset_and_enable(fd);
    }

    let started = Instant::now();
    std::thread::sleep(SAMPLE_WINDOW);
    let elapsed = started.elapsed();

    for fd in [refs_fd, miss_fd].into_iter().flatten() {
        disable(fd);
    }

    let references = match references {
        Ok(fd) => read_count(fd),
        Err(OpenError::Unavailable(msg)) => PlatformResult::Unavailable(msg),
        Err(OpenError::Permission(msg)) => PlatformResult::PermissionRequired(msg),
        Err(OpenError::Error(msg)) => PlatformResult::Error(DiagnosticError::PlatformError(msg)),
    };
    let misses = match misses {
        Ok(fd) => read_count(fd),
        Err(OpenError::Unavailable(msg)) => PlatformResult::Unavailable(msg),
        Err(OpenError::Permission(msg)) => PlatformResult::PermissionRequired(msg),
        Err(OpenError::Error(msg)) => PlatformResult::Error(DiagnosticError::PlatformError(msg)),
    };

    measured(CacheCounters {
        references,
        misses,
        window_nanos: measured(u64::try_from(elapsed.as_nanos()).unwrap_or(u64::MAX)),
    })
}

#[derive(Debug)]
enum OpenError {
    Permission(String),
    Unavailable(String),
    Error(String),
}

struct CounterFd(OwnedFd);

impl CounterFd {
    fn as_raw_borrow(&self) -> libc::c_int {
        use std::os::fd::AsRawFd;
        self.0.as_raw_fd()
    }
}

fn open_counter(config: u64) -> Result<CounterFd, OpenError> {
    let mut attr = PerfEventAttr {
        type_: PERF_TYPE_HARDWARE,
        size: mem::size_of::<PerfEventAttr>() as u32,
        config,
        // disabled | exclude_hv  (bit0 | bit5); enable explicitly after open.
        flags: 1 | (1 << 5),
        ..Default::default()
    };
    // SAFETY: perf_event_open with pid=0 (current process), cpu=-1 (any), no group.
    let fd = unsafe {
        libc::syscall(
            libc::SYS_perf_event_open,
            &mut attr as *mut PerfEventAttr,
            0_i32,
            -1_i32,
            -1_i32,
            0_u64,
        )
    };
    if fd < 0 {
        let err = io::Error::last_os_error();
        return Err(match err.raw_os_error() {
            Some(libc::EPERM) | Some(libc::EACCES) => {
                OpenError::Permission(format!("perf_event_open denied for config {config}: {err}"))
            }
            Some(libc::ENOENT)
            | Some(libc::ENODEV)
            | Some(libc::ENOTSUP)
            | Some(libc::EOPNOTSUPP) => OpenError::Unavailable(format!(
                "hardware cache counter {config} unavailable: {err}"
            )),
            _ => OpenError::Error(format!("perf_event_open({config}) failed: {err}")),
        });
    }
    // SAFETY: fd is a freshly opened valid descriptor owned uniquely here.
    Ok(CounterFd(unsafe { OwnedFd::from_raw_fd(fd as i32) }))
}

fn reset_and_enable(fd: libc::c_int) {
    unsafe {
        libc::ioctl(fd, PERF_EVENT_IOC_RESET, 0);
        libc::ioctl(fd, PERF_EVENT_IOC_ENABLE, 0);
    }
}

fn disable(fd: libc::c_int) {
    unsafe {
        libc::ioctl(fd, PERF_EVENT_IOC_DISABLE, 0);
    }
}

fn read_count(fd: CounterFd) -> PlatformResult<u64> {
    let raw = fd.0.into_raw_fd();
    let mut value = 0_u64;
    // SAFETY: read exactly one u64 counter value from a disabled perf fd.
    let n = unsafe { libc::read(raw, (&mut value as *mut u64).cast(), mem::size_of::<u64>()) };
    unsafe {
        libc::close(raw);
    }
    if n != mem::size_of::<u64>() as isize {
        let err = io::Error::last_os_error();
        return PlatformResult::Error(DiagnosticError::PlatformError(format!(
            "perf counter read failed: {err}"
        )));
    }
    measured(value)
}

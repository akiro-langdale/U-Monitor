//! Linux other-thread registers via ptrace(PTRACE_ATTACH / GETREGSET).

use std::{io, mem, ptr, time::SystemTime};
use u_monitor_core::{
    DataSource, DiagnosticError, Measurement, PlatformResult, ProcessId, RegisterArchitecture,
    RegisterSnapshot, RegisterValue, ThreadId,
};

fn measured<T>(value: T) -> PlatformResult<T> {
    PlatformResult::Success(Measurement {
        value,
        source: DataSource::Measured,
        timestamp: SystemTime::now().into(),
    })
}

fn reg(name: &str, bits: u64) -> RegisterValue {
    RegisterValue::new(name, bits, 64)
}

pub(super) fn capture_thread(
    process_id: ProcessId,
    thread_id: ThreadId,
) -> PlatformResult<RegisterSnapshot> {
    let tid = thread_id as libc::pid_t;
    let _ = process_id;
    // SAFETY: PTRACE_ATTACH stops the target thread until we detach.
    let attach = unsafe { libc::ptrace(libc::PTRACE_ATTACH, tid, 0, 0) };
    if attach != 0 {
        let err = io::Error::last_os_error();
        return match err.raw_os_error() {
            Some(libc::EPERM) | Some(libc::EACCES) => {
                PlatformResult::PermissionRequired(format!("ptrace attach denied: {err}"))
            }
            _ => PlatformResult::Error(DiagnosticError::PlatformError(format!(
                "ptrace attach failed: {err}"
            ))),
        };
    }

    let mut status = 0;
    // SAFETY: waitpid waits for the attached stop.
    let waited = unsafe { libc::waitpid(tid, &mut status, 0) };
    if waited < 0 {
        let err = io::Error::last_os_error();
        let _ = unsafe { libc::ptrace(libc::PTRACE_DETACH, tid, 0, 0) };
        return PlatformResult::Error(DiagnosticError::PlatformError(format!(
            "waitpid after ptrace failed: {err}"
        )));
    }

    let snapshot = read_regs(tid, thread_id);
    let _ = unsafe { libc::ptrace(libc::PTRACE_DETACH, tid, 0, 0) };
    snapshot
}

#[cfg(target_arch = "x86_64")]
fn read_regs(tid: libc::pid_t, thread_id: ThreadId) -> PlatformResult<RegisterSnapshot> {
    let mut regs: libc::user_regs_struct = unsafe { mem::zeroed() };
    let mut iov = libc::iovec {
        iov_base: ptr::addr_of_mut!(regs).cast(),
        iov_len: mem::size_of_val(&regs),
    };
    // SAFETY: GETREGSET fills user_regs_struct for a stopped thread.
    let rc = unsafe {
        libc::ptrace(
            libc::PTRACE_GETREGSET,
            tid,
            libc::NT_PRSTATUS as *const libc::c_void,
            &mut iov as *mut libc::iovec as *mut libc::c_void,
        )
    };
    if rc != 0 {
        let err = io::Error::last_os_error();
        return PlatformResult::Error(DiagnosticError::PlatformError(format!(
            "PTRACE_GETREGSET failed: {err}"
        )));
    }
    measured(RegisterSnapshot {
        architecture: RegisterArchitecture::X86_64,
        thread_id: measured(thread_id),
        registers: vec![
            reg("RAX", regs.rax),
            reg("RBX", regs.rbx),
            reg("RCX", regs.rcx),
            reg("RDX", regs.rdx),
            reg("RSI", regs.rsi),
            reg("RDI", regs.rdi),
            reg("RBP", regs.rbp),
            reg("RSP", regs.rsp),
            reg("R8", regs.r8),
            reg("R9", regs.r9),
            reg("R10", regs.r10),
            reg("R11", regs.r11),
            reg("R12", regs.r12),
            reg("R13", regs.r13),
            reg("R14", regs.r14),
            reg("R15", regs.r15),
            reg("RIP", regs.rip),
            reg("EFLAGS", regs.eflags),
        ],
    })
}

#[cfg(target_arch = "aarch64")]
fn read_regs(tid: libc::pid_t, thread_id: ThreadId) -> PlatformResult<RegisterSnapshot> {
    let mut regs: libc::user_regs_struct = unsafe { mem::zeroed() };
    let mut iov = libc::iovec {
        iov_base: ptr::addr_of_mut!(regs).cast(),
        iov_len: mem::size_of_val(&regs),
    };
    let rc = unsafe {
        libc::ptrace(
            libc::PTRACE_GETREGSET,
            tid,
            libc::NT_PRSTATUS as *const libc::c_void,
            &mut iov as *mut libc::iovec as *mut libc::c_void,
        )
    };
    if rc != 0 {
        let err = io::Error::last_os_error();
        return PlatformResult::Error(DiagnosticError::PlatformError(format!(
            "PTRACE_GETREGSET failed: {err}"
        )));
    }
    let mut registers = Vec::with_capacity(34);
    for (index, value) in regs.regs.iter().enumerate() {
        registers.push(reg(&format!("X{index}"), *value));
    }
    registers.push(reg("SP", regs.sp));
    registers.push(reg("PC", regs.pc));
    registers.push(reg("PSTATE", regs.pstate));
    measured(RegisterSnapshot {
        architecture: RegisterArchitecture::AArch64,
        thread_id: measured(thread_id),
        registers,
    })
}

#[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
fn read_regs(_tid: libc::pid_t, _thread_id: ThreadId) -> PlatformResult<RegisterSnapshot> {
    PlatformResult::NotImplemented(
        "Linux ptrace register capture supports x86_64 and aarch64 only".into(),
    )
}

//! Windows x86-64 register snapshot via RtlCaptureContext.

use std::time::SystemTime;
use u_monitor_core::{
    DataSource, Measurement, PlatformResult, RegisterArchitecture, RegisterSnapshot, RegisterValue,
};
use windows::Win32::System::Diagnostics::Debug::{CONTEXT, CONTEXT_FULL_AMD64, RtlCaptureContext};
use windows::Win32::System::Threading::GetCurrentThreadId;

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

fn reg32(name: &str, bits: u32) -> RegisterValue {
    RegisterValue::new(name, u64::from(bits), 32)
}

fn reg16(name: &str, bits: u16) -> RegisterValue {
    RegisterValue::new(name, u64::from(bits), 16)
}

pub(super) fn capture() -> PlatformResult<RegisterSnapshot> {
    let mut context = CONTEXT {
        ContextFlags: CONTEXT_FULL_AMD64,
        ..Default::default()
    };
    // SAFETY: RtlCaptureContext writes a complete CONTEXT for the calling thread.
    unsafe {
        RtlCaptureContext(&mut context);
    }
    // SAFETY: GetCurrentThreadId is a pure kernel32 query with no pointer args.
    let thread_id = u64::from(unsafe { GetCurrentThreadId() });

    let registers = vec![
        reg("RAX", context.Rax),
        reg("RBX", context.Rbx),
        reg("RCX", context.Rcx),
        reg("RDX", context.Rdx),
        reg("RSI", context.Rsi),
        reg("RDI", context.Rdi),
        reg("RBP", context.Rbp),
        reg("RSP", context.Rsp),
        reg("R8", context.R8),
        reg("R9", context.R9),
        reg("R10", context.R10),
        reg("R11", context.R11),
        reg("R12", context.R12),
        reg("R13", context.R13),
        reg("R14", context.R14),
        reg("R15", context.R15),
        reg("RIP", context.Rip),
        reg32("RFLAGS", context.EFlags),
        reg16("CS", context.SegCs),
        reg16("DS", context.SegDs),
        reg16("ES", context.SegEs),
        reg16("FS", context.SegFs),
        reg16("GS", context.SegGs),
        reg16("SS", context.SegSs),
    ];

    measured(RegisterSnapshot {
        architecture: RegisterArchitecture::X86_64,
        thread_id: measured(thread_id),
        registers,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capture_returns_x86_64_gprs_with_nonzero_rip_or_rsp() {
        let PlatformResult::Success(snapshot) = capture() else {
            panic!("expected register snapshot");
        };
        assert_eq!(snapshot.value.architecture, RegisterArchitecture::X86_64);
        assert!(matches!(
            snapshot.value.thread_id,
            PlatformResult::Success(_)
        ));
        let names: Vec<_> = snapshot
            .value
            .registers
            .iter()
            .map(|reg| reg.name.as_str())
            .collect();
        for expected in ["RAX", "RBX", "RSP", "RIP", "RFLAGS"] {
            assert!(names.contains(&expected), "missing {expected}");
        }
        let rsp = snapshot
            .value
            .registers
            .iter()
            .find(|reg| reg.name == "RSP")
            .unwrap()
            .bits;
        let rip = snapshot
            .value
            .registers
            .iter()
            .find(|reg| reg.name == "RIP")
            .unwrap()
            .bits;
        assert!(rsp != 0 || rip != 0);
    }
}

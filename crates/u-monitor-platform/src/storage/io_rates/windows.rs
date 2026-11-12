//! Aggregate `IOCTL_DISK_PERFORMANCE` counters across physical drives.

use std::mem;
use u_monitor_core::{DiagnosticError, DiskIoRates, PlatformResult};
use windows::Win32::Foundation::{
    CloseHandle, ERROR_ACCESS_DENIED, ERROR_FILE_NOT_FOUND, ERROR_PATH_NOT_FOUND, GENERIC_READ,
    HANDLE,
};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FILE_FLAGS_AND_ATTRIBUTES, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
};
use windows::Win32::System::IO::DeviceIoControl;
use windows::Win32::System::Ioctl::{DISK_PERFORMANCE, IOCTL_DISK_PERFORMANCE};
use windows::core::PCWSTR;

pub(super) fn cumulative_bytes() -> Result<(u64, u64), PlatformResult<DiskIoRates>> {
    let mut read_total = 0_u64;
    let mut write_total = 0_u64;
    let mut seen = 0_u32;
    let mut denied = 0_u32;

    for index in 0..16 {
        match read_drive(index) {
            Ok((read, write)) => {
                read_total = read_total.saturating_add(read);
                write_total = write_total.saturating_add(write);
                seen = seen.saturating_add(1);
            }
            Err(DriveError::Denied) => denied = denied.saturating_add(1),
            Err(DriveError::Missing) => {}
            Err(DriveError::Other(message)) => {
                return Err(PlatformResult::Error(DiagnosticError::PlatformError(
                    message,
                )));
            }
        }
    }

    if seen == 0 {
        if denied > 0 {
            return Err(PlatformResult::PermissionRequired(format!(
                "IOCTL_DISK_PERFORMANCE denied for {denied} physical drive(s)"
            )));
        }
        return Err(PlatformResult::Unavailable(
            "No physical drives answered IOCTL_DISK_PERFORMANCE".into(),
        ));
    }
    Ok((read_total, write_total))
}

enum DriveError {
    Missing,
    Denied,
    Other(String),
}

fn read_drive(index: u32) -> Result<(u64, u64), DriveError> {
    let path = format!(r"\\.\PhysicalDrive{index}");
    let wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
    let handle = unsafe {
        CreateFileW(
            PCWSTR(wide.as_ptr()),
            GENERIC_READ.0,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            None,
            OPEN_EXISTING,
            FILE_FLAGS_AND_ATTRIBUTES(0),
            None,
        )
    };
    let handle = match handle {
        Ok(handle) if !handle.is_invalid() => handle,
        Ok(_) => return Err(DriveError::Missing),
        Err(error) => {
            if error.code() == ERROR_FILE_NOT_FOUND.into()
                || error.code() == ERROR_PATH_NOT_FOUND.into()
            {
                return Err(DriveError::Missing);
            }
            if error.code() == ERROR_ACCESS_DENIED.into() {
                return Err(DriveError::Denied);
            }
            return Err(DriveError::Other(format!(
                "CreateFileW({path}) failed: {error}"
            )));
        }
    };

    let result = read_performance(handle);
    let _ = unsafe { CloseHandle(handle) };
    result
}

fn read_performance(handle: HANDLE) -> Result<(u64, u64), DriveError> {
    let mut performance = DISK_PERFORMANCE::default();
    let mut returned = 0_u32;
    let ok = unsafe {
        DeviceIoControl(
            handle,
            IOCTL_DISK_PERFORMANCE,
            None,
            0,
            Some((&mut performance as *mut DISK_PERFORMANCE).cast()),
            mem::size_of::<DISK_PERFORMANCE>() as u32,
            Some(&mut returned),
            None,
        )
    };
    if let Err(error) = ok {
        if error.code() == ERROR_ACCESS_DENIED.into() {
            return Err(DriveError::Denied);
        }
        return Err(DriveError::Other(format!(
            "DeviceIoControl(IOCTL_DISK_PERFORMANCE) failed: {error}"
        )));
    }
    Ok((
        performance.BytesRead.max(0) as u64,
        performance.BytesWritten.max(0) as u64,
    ))
}

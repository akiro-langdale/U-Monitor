//! Current-thread register capture. Other threads are intentionally out of scope.
#[cfg(all(windows, target_arch = "x86_64"))]
mod windows_x64;

use u_monitor_core::{PlatformResult, RegisterSnapshot};

pub(crate) fn capture_current_thread() -> PlatformResult<RegisterSnapshot> {
    #[cfg(all(windows, target_arch = "x86_64"))]
    {
        windows_x64::capture()
    }
    #[cfg(not(all(windows, target_arch = "x86_64")))]
    {
        PlatformResult::NotImplemented(
            "Current-thread register capture is implemented for Windows x86-64 in this version"
                .into(),
        )
    }
}

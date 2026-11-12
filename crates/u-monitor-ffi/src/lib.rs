//! Stable C ABI for U-Monitor.
//!
//! Ownership rules:
//! - Caller-owned output structs are filled by the library and need no free.
//! - Opaque handles (`UmHierarchy`) are created with `*_create` and must be
//!   released with the matching `*_destroy`.
//! - String outputs written into caller buffers are UTF-8 without a BOM and are
//!   always NUL-terminated when `capacity > 0`.
//!
//! Values never invent MEASURED data. Simulation entry points are always
//! labelled `UM_SOURCE_SIMULATED`.
//!
//! Pointer arguments are validated for NULL then dereferenced. That pattern is
//! intentional for a C ABI and is exempt from `clippy::not_unsafe_ptr_arg_deref`.
#![allow(clippy::not_unsafe_ptr_arg_deref)]

mod cache;
mod convert;
mod hierarchy;
mod identity;
mod memory_sample;
mod ram;
mod status;

pub use cache::*;
pub use convert::{UmFieldU32, UmFieldU64};
pub use hierarchy::*;
pub use identity::*;
pub use memory_sample::*;
pub use ram::*;
pub use status::*;

use std::os::raw::c_char;

/// Library semantic version string (`MAJOR.MINOR.PATCH`), static storage.
#[unsafe(no_mangle)]
pub extern "C" fn um_version() -> *const c_char {
    // Keep in sync with `[workspace.package] version`.
    concat!(env!("CARGO_PKG_VERSION"), "\0").as_ptr().cast()
}

/// Writes a short English description of `status` into `buffer`.
///
/// Returns [`UmStatus::Ok`] on success, [`UmStatus::NullArgument`] when `buffer`
/// is null, or [`UmStatus::BufferTooSmall`] when `capacity` cannot hold the text.
#[unsafe(no_mangle)]
pub extern "C" fn um_status_name(
    status: UmStatus,
    buffer: *mut c_char,
    capacity: usize,
) -> UmStatus {
    convert::write_cstr(status.name(), buffer, capacity)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CStr;

    #[test]
    fn version_is_nul_terminated_static() {
        let version = unsafe { CStr::from_ptr(um_version()) };
        assert_eq!(version.to_str().unwrap(), env!("CARGO_PKG_VERSION"));
    }
}

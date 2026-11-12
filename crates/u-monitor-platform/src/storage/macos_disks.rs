//! macOS disk-kind enrichment notes.
//!
//! Mounted volume enumeration still goes through `sysinfo` (Disk Arbitration under the hood on
//! supported hosts). This module documents that DiskStorage on macOS is volume discovery only;
//! IOCTL-style physical counters remain Unavailable here.

#![allow(dead_code)]

/// Human-readable capability note for Overview / docs.
pub(super) const DISCOVERY_NOTE: &str =
    "macOS volume discovery uses sysinfo/Disk Arbitration; physical IOCTL counters are unavailable";

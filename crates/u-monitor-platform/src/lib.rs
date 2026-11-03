//! Native adapter boundary. Hardware discovery is not implemented yet.

use u_monitor_core::{Availability, PlatformAdapter, PlatformFeature};

/// A stub platform adapter (template for future OS-specific implementations)
#[derive(Debug, Default)]
pub struct StubPlatformAdapter;

impl StubPlatformAdapter {
    pub fn new() -> Self {
        Self
    }
}

impl PlatformAdapter for StubPlatformAdapter {
    fn name(&self) -> &'static str {
        "Stub Platform Adapter"
    }

    fn check_availability(&self, _feature: PlatformFeature) -> Availability {
        // Honestly return NotImplemented for all functions at this stage
        Availability::NotImplemented
    }
}

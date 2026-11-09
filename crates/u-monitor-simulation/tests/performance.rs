//! Lightweight performance guard for the educational hierarchy model.

use std::time::{Duration, Instant};
use u_monitor_core::DataSource;
use u_monitor_simulation::HierarchySimulator;

#[test]
fn hundred_thousand_accesses_stay_under_two_seconds() {
    let mut sim = HierarchySimulator::new_default();
    let started = Instant::now();
    for index in 0..100_000u64 {
        // Stride across lines to exercise miss/hit paths, not a micro-benchmark harness.
        let measurement = sim.access(index.wrapping_mul(64));
        assert_eq!(measurement.source, DataSource::Simulated);
    }
    let elapsed = started.elapsed();
    assert!(
        elapsed < Duration::from_secs(2),
        "hierarchy model too slow: {elapsed:?}"
    );
    assert_eq!(sim.stats().value.accesses, 100_000);
}

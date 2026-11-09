//! Independent memory-hierarchy models. Every result is SIMULATED — never mixed with MEASURED data.
//!
//! This crate models educational cache behaviour (set-associative LRU) and a separate disk latency
//! toy model. Latencies and hits/misses come from the model configuration, not from hardware
//! counters or OS APIs. Topology seeding only copies metadata sizes/ways into SIMULATED params.

mod disk;
mod hierarchy;

pub use disk::{DiskAccessResult, DiskLatencyConfig, DiskLatencyModel};
pub use hierarchy::{
    AccessEvent, AccessResult, AccessStep, DEFAULT_DEMO_ADDRESSES, HierarchyConfig,
    HierarchySimulator, HierarchyStats, LevelConfig, MemoryLevel, SeededHierarchyConfig,
};

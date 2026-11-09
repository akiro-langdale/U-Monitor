//! Independent memory-hierarchy models. Every result is SIMULATED — never mixed with MEASURED data.
//!
//! This crate models educational cache behaviour (set-associative LRU). Latencies and hits/misses
//! come from the model configuration, not from hardware counters or OS APIs.

mod hierarchy;

pub use hierarchy::{
    AccessEvent, AccessResult, AccessStep, DEFAULT_DEMO_ADDRESSES, HierarchyConfig,
    HierarchySimulator, HierarchyStats, LevelConfig, MemoryLevel,
};

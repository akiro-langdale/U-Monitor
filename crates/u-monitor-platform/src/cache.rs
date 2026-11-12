//! CPU cache topology and counters via U-Library.

use std::time::SystemTime;
use u_library_cache::{self, CacheError, CacheKind as LibKind, CacheNode as LibNode, CacheTopology as LibTopology};
use u_monitor_core::{
    CacheCounters, CacheKind, CacheNode, CacheTopology, DataSource, DiagnosticError, Measurement,
    PlatformResult,
};

fn measured<T>(value: T) -> PlatformResult<T> {
    PlatformResult::Success(Measurement {
        value,
        source: DataSource::Measured,
        timestamp: SystemTime::now().into(),
    })
}

fn map_kind(kind: LibKind) -> CacheKind {
    match kind {
        LibKind::Data => CacheKind::Data,
        LibKind::Instruction => CacheKind::Instruction,
        LibKind::Unified => CacheKind::Unified,
        LibKind::Trace => CacheKind::Trace,
        LibKind::Unknown => CacheKind::Unknown,
    }
}

fn adapt_node(node: LibNode) -> CacheNode {
    CacheNode {
        level: node.level,
        kind: map_kind(node.kind),
        size_bytes: node.size_bytes,
        line_size_bytes: node.line_size_bytes,
        associativity: node.associativity,
        fully_associative: node.fully_associative,
        shared_logical_processors: node.shared_logical_processors,
    }
}

fn adapt_topology(topology: LibTopology) -> CacheTopology {
    CacheTopology {
        nodes: topology.nodes.into_iter().map(adapt_node).collect(),
    }
}

fn map_topology_error(error: CacheError) -> PlatformResult<CacheTopology> {
    match error {
        CacheError::Unsupported(message) => PlatformResult::NotImplemented(message),
        CacheError::Unavailable(message) => PlatformResult::Unavailable(message),
        CacheError::PermissionRequired(message) => PlatformResult::PermissionRequired(message),
        CacheError::Platform(message) => {
            PlatformResult::Error(DiagnosticError::PlatformError(message))
        }
    }
}

fn map_counter_field<T: Copy>(
    value: Option<T>,
    unavailable: &'static str,
) -> PlatformResult<T> {
    match value {
        Some(value) => measured(value),
        None => PlatformResult::Unavailable(unavailable.into()),
    }
}

fn map_counters_error(error: CacheError) -> PlatformResult<CacheCounters> {
    match error {
        CacheError::Unsupported(message) => PlatformResult::NotImplemented(message),
        CacheError::Unavailable(message) => PlatformResult::Unavailable(message),
        CacheError::PermissionRequired(message) => PlatformResult::PermissionRequired(message),
        CacheError::Platform(message) => {
            PlatformResult::Error(DiagnosticError::PlatformError(message))
        }
    }
}

pub(crate) fn discover() -> PlatformResult<CacheTopology> {
    match u_library_cache::discover() {
        Ok(topology) => measured(adapt_topology(topology)),
        Err(error) => map_topology_error(error),
    }
}

pub(crate) fn sample_counters() -> PlatformResult<CacheCounters> {
    match u_library_cache::sample_counters() {
        Ok(counters) => measured(CacheCounters {
            references: map_counter_field(
                counters.references,
                "hardware cache references unavailable for this sample",
            ),
            misses: map_counter_field(
                counters.misses,
                "hardware cache misses unavailable for this sample",
            ),
            window_nanos: map_counter_field(counters.window_nanos, "sample window unavailable"),
        }),
        Err(error) => map_counters_error(error),
    }
}

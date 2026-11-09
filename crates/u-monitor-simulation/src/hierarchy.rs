//! Set-associative LRU hierarchy: L1 → L2 → RAM. Values are model outputs only.

use std::time::SystemTime;
use u_monitor_core::{
    CacheKind, CacheNode, CacheTopology, DataSource, DiagnosticError, Measurement, PlatformResult,
};

/// Logical level visited while resolving an address in the model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryLevel {
    L1,
    L2,
    Ram,
}

impl MemoryLevel {
    pub fn label(self) -> &'static str {
        match self {
            Self::L1 => "L1",
            Self::L2 => "L2",
            Self::Ram => "RAM",
        }
    }
}

/// One discrete event on the simulated access path (for UI animation).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessEvent {
    Probe,
    Hit,
    Miss,
    Fetch,
    Fill,
    Evict,
}

impl AccessEvent {
    pub fn label(self) -> &'static str {
        match self {
            Self::Probe => "PROBE",
            Self::Hit => "HIT",
            Self::Miss => "MISS",
            Self::Fetch => "FETCH",
            Self::Fill => "FILL",
            Self::Evict => "EVICT",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AccessStep {
    pub level: MemoryLevel,
    pub event: AccessEvent,
}

/// Result of one modelled memory access.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccessResult {
    pub address: u64,
    pub line_address: u64,
    pub resolved_at: MemoryLevel,
    pub hit: bool,
    pub latency_ns: u64,
    pub steps: Vec<AccessStep>,
}

/// Aggregate counters for the current simulator instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct HierarchyStats {
    pub accesses: u64,
    pub l1_hits: u64,
    pub l2_hits: u64,
    pub ram_fills: u64,
    pub evictions: u64,
    pub total_latency_ns: u64,
}

/// Parameters for one cache level in the model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LevelConfig {
    pub sets: usize,
    pub ways: usize,
    pub line_size_bytes: u32,
    pub hit_latency_ns: u64,
}

impl LevelConfig {
    pub fn validate(self) -> Result<(), DiagnosticError> {
        if self.sets == 0 || self.ways == 0 {
            return Err(DiagnosticError::SimulationError(
                "cache sets and ways must be non-zero".into(),
            ));
        }
        if self.line_size_bytes == 0 || !self.line_size_bytes.is_power_of_two() {
            return Err(DiagnosticError::SimulationError(
                "line size must be a non-zero power of two".into(),
            ));
        }
        Ok(())
    }
}

/// Full hierarchy configuration used by [`HierarchySimulator`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HierarchyConfig {
    pub l1: LevelConfig,
    pub l2: LevelConfig,
    pub ram_latency_ns: u64,
}

impl Default for HierarchyConfig {
    fn default() -> Self {
        Self {
            l1: LevelConfig {
                sets: 64,
                ways: 4,
                line_size_bytes: 64,
                hit_latency_ns: 4,
            },
            l2: LevelConfig {
                sets: 512,
                ways: 8,
                line_size_bytes: 64,
                hit_latency_ns: 12,
            },
            ram_latency_ns: 80,
        }
    }
}

impl HierarchyConfig {
    pub fn validate(self) -> Result<(), DiagnosticError> {
        self.l1.validate()?;
        self.l2.validate()?;
        if self.l1.line_size_bytes != self.l2.line_size_bytes {
            return Err(DiagnosticError::SimulationError(
                "L1 and L2 line sizes must match in this model".into(),
            ));
        }
        if self.ram_latency_ns == 0 {
            return Err(DiagnosticError::SimulationError(
                "RAM latency must be non-zero".into(),
            ));
        }
        Ok(())
    }

    /// Seeds educational defaults from MEASURED topology metadata when present.
    /// Missing fields keep defaults; the result remains a SIMULATED model config.
    pub fn from_cache_topology(topology: &CacheTopology) -> SeededHierarchyConfig {
        let mut config = Self::default();
        let mut notes = Vec::new();
        let mut used_measured = false;

        if let Some(line) = topology
            .nodes
            .iter()
            .filter_map(|node| node.line_size_bytes)
            .find(|size| *size > 0 && size.is_power_of_two())
        {
            config.l1.line_size_bytes = line;
            config.l2.line_size_bytes = line;
            notes.push(format!("line size seeded from MEASURED topology: {line} B"));
            used_measured = true;
        }

        if let Some(l1) = pick_level(topology, 1)
            && let Some(level) = level_from_node(l1, config.l1)
        {
            config.l1 = level;
            notes.push(format!(
                "L1 seeded from MEASURED node ({} B, {}-way)",
                l1.size_bytes.unwrap_or(0),
                config.l1.ways
            ));
            used_measured = true;
        }
        if let Some(l2) = pick_level(topology, 2)
            && let Some(level) = level_from_node(l2, config.l2)
        {
            config.l2 = level;
            notes.push(format!(
                "L2 seeded from MEASURED node ({} B, {}-way)",
                l2.size_bytes.unwrap_or(0),
                config.l2.ways
            ));
            used_measured = true;
        }

        if !used_measured {
            notes.push(
                "MEASURED topology lacked usable L1/L2 metadata; using model defaults".into(),
            );
        }
        // Align line sizes after partial seeding.
        if config.l1.line_size_bytes != config.l2.line_size_bytes {
            config.l2.line_size_bytes = config.l1.line_size_bytes;
            notes.push("aligned L2 line size to L1 for this model".into());
        }
        if config.validate().is_err() {
            notes.push("seeded config invalid; falling back to defaults".into());
            return SeededHierarchyConfig {
                config: Self::default(),
                seeded_from_measured: false,
                notes,
            };
        }
        SeededHierarchyConfig {
            config,
            seeded_from_measured: used_measured,
            notes,
        }
    }
}

/// Result of seeding [`HierarchyConfig`] from measured cache topology.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeededHierarchyConfig {
    pub config: HierarchyConfig,
    pub seeded_from_measured: bool,
    pub notes: Vec<String>,
}

fn pick_level(topology: &CacheTopology, level: u32) -> Option<&CacheNode> {
    topology.nodes.iter().find(|node| {
        node.level == level
            && matches!(
                node.kind,
                CacheKind::Data | CacheKind::Unified | CacheKind::Unknown
            )
            && node.size_bytes.is_some_and(|size| size > 0)
    })
}

fn level_from_node(node: &CacheNode, defaults: LevelConfig) -> Option<LevelConfig> {
    let size = node.size_bytes?;
    let line = node
        .line_size_bytes
        .filter(|size| *size > 0 && size.is_power_of_two())
        .unwrap_or(defaults.line_size_bytes);
    let ways = if node.fully_associative {
        // Cap fully-associative models so the educational simulator stays tractable.
        ((size / u64::from(line)).clamp(1, 64)) as usize
    } else {
        node.associativity
            .filter(|ways| *ways > 0)
            .map(|ways| ways as usize)
            .unwrap_or(defaults.ways)
            .clamp(1, 64)
    };
    let sets = (size / (u64::from(line) * ways as u64)).max(1) as usize;
    Some(LevelConfig {
        sets: sets.clamp(1, 4096),
        ways,
        line_size_bytes: line,
        hit_latency_ns: defaults.hit_latency_ns,
    })
}

/// Demo addresses for the UI: stride across lines to exercise hits and misses.
pub const DEFAULT_DEMO_ADDRESSES: &[u64] = &[
    0x1000, 0x1040, 0x1080, 0x1000, 0x2000, 0x1040, 0x3000, 0x1000, 0x1080, 0x4000,
];

#[derive(Debug, Clone)]
struct CacheLine {
    tag: u64,
    valid: bool,
    /// Higher means more recently used within the set.
    lru: u64,
}

#[derive(Debug, Clone)]
struct CacheLevel {
    config: LevelConfig,
    sets: Vec<Vec<CacheLine>>,
    clock: u64,
}

impl CacheLevel {
    fn new(config: LevelConfig) -> Self {
        let sets = (0..config.sets)
            .map(|_| {
                (0..config.ways)
                    .map(|_| CacheLine {
                        tag: 0,
                        valid: false,
                        lru: 0,
                    })
                    .collect()
            })
            .collect();
        Self {
            config,
            sets,
            clock: 0,
        }
    }

    fn line_address(&self, address: u64) -> u64 {
        address & !(u64::from(self.config.line_size_bytes) - 1)
    }

    fn index_and_tag(&self, line_address: u64) -> (usize, u64) {
        let line_number = line_address / u64::from(self.config.line_size_bytes);
        let index = (line_number % self.config.sets as u64) as usize;
        let tag = line_number / self.config.sets as u64;
        (index, tag)
    }

    fn probe(&mut self, line_address: u64) -> bool {
        let (index, tag) = self.index_and_tag(line_address);
        self.clock = self.clock.saturating_add(1);
        if let Some(line) = self.sets[index]
            .iter_mut()
            .find(|line| line.valid && line.tag == tag)
        {
            line.lru = self.clock;
            true
        } else {
            false
        }
    }

    /// Inserts `line_address`, returning whether an existing valid line was evicted.
    fn fill(&mut self, line_address: u64) -> bool {
        let (index, tag) = self.index_and_tag(line_address);
        self.clock = self.clock.saturating_add(1);
        if let Some(line) = self.sets[index].iter_mut().find(|line| !line.valid) {
            line.valid = true;
            line.tag = tag;
            line.lru = self.clock;
            return false;
        }
        let victim = self.sets[index]
            .iter()
            .enumerate()
            .min_by_key(|(_, line)| line.lru)
            .map(|(idx, _)| idx)
            .expect("ways is non-zero");
        let line = &mut self.sets[index][victim];
        let evicted = line.valid;
        line.valid = true;
        line.tag = tag;
        line.lru = self.clock;
        evicted
    }
}

/// Educational L1/L2/RAM simulator. Not a CPU emulator.
#[derive(Debug, Clone)]
pub struct HierarchySimulator {
    config: HierarchyConfig,
    l1: CacheLevel,
    l2: CacheLevel,
    stats: HierarchyStats,
}

impl HierarchySimulator {
    pub fn try_new(config: HierarchyConfig) -> Result<Self, DiagnosticError> {
        config.validate()?;
        Ok(Self {
            l1: CacheLevel::new(config.l1),
            l2: CacheLevel::new(config.l2),
            config,
            stats: HierarchyStats::default(),
        })
    }

    pub fn new_default() -> Self {
        Self::try_new(HierarchyConfig::default()).expect("default hierarchy config is valid")
    }

    pub fn config(&self) -> HierarchyConfig {
        self.config
    }

    pub fn stats(&self) -> Measurement<HierarchyStats> {
        simulated(self.stats)
    }

    pub fn reset(&mut self) {
        self.l1 = CacheLevel::new(self.config.l1);
        self.l2 = CacheLevel::new(self.config.l2);
        self.stats = HierarchyStats::default();
    }

    pub fn reconfigure(&mut self, config: HierarchyConfig) -> PlatformResult<()> {
        if let Err(error) = config.validate() {
            return PlatformResult::Error(error);
        }
        self.config = config;
        self.reset();
        PlatformResult::Success(simulated(()))
    }

    /// Models one load. Outcome and latency are SIMULATED model values.
    pub fn access(&mut self, address: u64) -> Measurement<AccessResult> {
        let line_address = self.l1.line_address(address);
        let mut steps = Vec::new();

        steps.push(AccessStep {
            level: MemoryLevel::L1,
            event: AccessEvent::Probe,
        });
        if self.l1.probe(line_address) {
            steps.push(AccessStep {
                level: MemoryLevel::L1,
                event: AccessEvent::Hit,
            });
            let latency_ns = self.config.l1.hit_latency_ns;
            self.stats.accesses = self.stats.accesses.saturating_add(1);
            self.stats.l1_hits = self.stats.l1_hits.saturating_add(1);
            self.stats.total_latency_ns = self.stats.total_latency_ns.saturating_add(latency_ns);
            return simulated(AccessResult {
                address,
                line_address,
                resolved_at: MemoryLevel::L1,
                hit: true,
                latency_ns,
                steps,
            });
        }
        steps.push(AccessStep {
            level: MemoryLevel::L1,
            event: AccessEvent::Miss,
        });

        steps.push(AccessStep {
            level: MemoryLevel::L2,
            event: AccessEvent::Probe,
        });
        if self.l2.probe(line_address) {
            steps.push(AccessStep {
                level: MemoryLevel::L2,
                event: AccessEvent::Hit,
            });
            steps.push(AccessStep {
                level: MemoryLevel::L1,
                event: AccessEvent::Fill,
            });
            let evicted = self.l1.fill(line_address);
            if evicted {
                steps.push(AccessStep {
                    level: MemoryLevel::L1,
                    event: AccessEvent::Evict,
                });
                self.stats.evictions = self.stats.evictions.saturating_add(1);
            }
            let latency_ns = self.config.l2.hit_latency_ns;
            self.stats.accesses = self.stats.accesses.saturating_add(1);
            self.stats.l2_hits = self.stats.l2_hits.saturating_add(1);
            self.stats.total_latency_ns = self.stats.total_latency_ns.saturating_add(latency_ns);
            return simulated(AccessResult {
                address,
                line_address,
                resolved_at: MemoryLevel::L2,
                hit: false,
                latency_ns,
                steps,
            });
        }
        steps.push(AccessStep {
            level: MemoryLevel::L2,
            event: AccessEvent::Miss,
        });

        steps.push(AccessStep {
            level: MemoryLevel::Ram,
            event: AccessEvent::Fetch,
        });
        steps.push(AccessStep {
            level: MemoryLevel::L2,
            event: AccessEvent::Fill,
        });
        if self.l2.fill(line_address) {
            steps.push(AccessStep {
                level: MemoryLevel::L2,
                event: AccessEvent::Evict,
            });
            self.stats.evictions = self.stats.evictions.saturating_add(1);
        }
        steps.push(AccessStep {
            level: MemoryLevel::L1,
            event: AccessEvent::Fill,
        });
        if self.l1.fill(line_address) {
            steps.push(AccessStep {
                level: MemoryLevel::L1,
                event: AccessEvent::Evict,
            });
            self.stats.evictions = self.stats.evictions.saturating_add(1);
        }

        let latency_ns = self.config.ram_latency_ns;
        self.stats.accesses = self.stats.accesses.saturating_add(1);
        self.stats.ram_fills = self.stats.ram_fills.saturating_add(1);
        self.stats.total_latency_ns = self.stats.total_latency_ns.saturating_add(latency_ns);
        simulated(AccessResult {
            address,
            line_address,
            resolved_at: MemoryLevel::Ram,
            hit: false,
            latency_ns,
            steps,
        })
    }
}

fn simulated<T>(value: T) -> Measurement<T> {
    Measurement {
        value,
        source: DataSource::Simulated,
        timestamp: SystemTime::now().into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_is_valid_and_marks_simulated() {
        let mut sim = HierarchySimulator::new_default();
        let first = sim.access(0x1000);
        assert_eq!(first.source, DataSource::Simulated);
        assert!(!first.value.hit);
        assert_eq!(first.value.resolved_at, MemoryLevel::Ram);
        let second = sim.access(0x1000);
        assert!(second.value.hit);
        assert_eq!(second.value.resolved_at, MemoryLevel::L1);
        assert_eq!(sim.stats().source, DataSource::Simulated);
        assert_eq!(sim.stats().value.l1_hits, 1);
        assert_eq!(sim.stats().value.ram_fills, 1);
    }

    #[test]
    fn rejects_invalid_line_size_without_mutating_state() {
        let mut sim = HierarchySimulator::new_default();
        let _ = sim.access(0x40);
        let before = sim.stats().value;
        let bad = HierarchyConfig {
            l1: LevelConfig {
                line_size_bytes: 3,
                ..HierarchyConfig::default().l1
            },
            ..HierarchyConfig::default()
        };
        assert!(matches!(
            sim.reconfigure(bad),
            PlatformResult::Error(DiagnosticError::SimulationError(_))
        ));
        assert_eq!(sim.stats().value, before);
    }

    #[test]
    fn seeds_from_measured_topology_without_claiming_hardware_counters() {
        let topology = CacheTopology {
            nodes: vec![
                CacheNode {
                    level: 1,
                    kind: CacheKind::Data,
                    size_bytes: Some(32 * 1024),
                    line_size_bytes: Some(64),
                    associativity: Some(8),
                    fully_associative: false,
                    shared_logical_processors: vec![0],
                },
                CacheNode {
                    level: 2,
                    kind: CacheKind::Unified,
                    size_bytes: Some(256 * 1024),
                    line_size_bytes: Some(64),
                    associativity: Some(8),
                    fully_associative: false,
                    shared_logical_processors: vec![0, 1],
                },
            ],
        };
        let seeded = HierarchyConfig::from_cache_topology(&topology);
        assert!(seeded.seeded_from_measured);
        assert_eq!(seeded.config.l1.line_size_bytes, 64);
        assert_eq!(seeded.config.l1.sets, 64); // 32KiB / (64 * 8)
        assert_eq!(seeded.config.l2.sets, 512); // 256KiB / (64 * 8)
        let mut sim = HierarchySimulator::try_new(seeded.config).unwrap();
        assert_eq!(sim.access(0x1000).source, DataSource::Simulated);
    }

    #[test]
    fn l2_hit_fills_l1_without_ram() {
        let mut sim = HierarchySimulator::try_new(HierarchyConfig {
            l1: LevelConfig {
                sets: 1,
                ways: 1,
                line_size_bytes: 64,
                hit_latency_ns: 1,
            },
            l2: LevelConfig {
                sets: 1,
                ways: 2,
                line_size_bytes: 64,
                hit_latency_ns: 5,
            },
            ram_latency_ns: 50,
        })
        .unwrap();
        let _ = sim.access(0x0000);
        // Evict L1 by accessing another line that still fits in L2.
        let _ = sim.access(0x0040);
        let again = sim.access(0x0000);
        assert_eq!(again.value.resolved_at, MemoryLevel::L2);
        assert!(!again.value.hit);
        assert_eq!(again.value.latency_ns, 5);
        assert!(
            again
                .value
                .steps
                .iter()
                .any(|step| step.level == MemoryLevel::L2 && step.event == AccessEvent::Hit)
        );
        assert_eq!(sim.stats().value.ram_fills, 2);
        assert_eq!(sim.stats().value.l2_hits, 1);
    }
}

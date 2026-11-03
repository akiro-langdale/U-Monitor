//! Synchronous RAM sampling and bounded in-memory history.
//! Scheduling and worker threads are intentionally left to the caller.
use std::{
    collections::VecDeque,
    num::NonZeroUsize,
    time::{Duration, Instant, SystemTime},
};
use u_monitor_core::{DiagnosticError, PlatformAdapter, PlatformResult, RamInfo, Timestamp};

/// One attempt to obtain RAM data. Failed attempts remain in history as failures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TelemetrySnapshot {
    /// Starts at 1 and increases independently of history eviction.
    pub sequence: u64,
    /// UTC time when the collector completed this attempt. Adapter measurement
    /// timestamps remain untouched and may precede this value.
    pub captured_at: Timestamp,
    /// Monotonic time from collector creation to completion of this attempt.
    /// This is not a sampling interval or the duration of the adapter request.
    pub elapsed: Duration,
    pub ram: PlatformResult<RamInfo>,
}

/// Owns an adapter and retains at most capacity snapshots, oldest first.
/// The limit bounds record count, not exact allocation or variable-size payloads.
pub struct TelemetryCollector<A> {
    adapter: A,
    capacity: NonZeroUsize,
    history: VecDeque<TelemetrySnapshot>,
    started_at: Instant,
    last_sequence: u64,
}

impl<A: PlatformAdapter> TelemetryCollector<A> {
    /// Zero capacity is a configuration error, never silently replaced by a default.
    pub fn new(adapter: A, capacity: usize) -> Result<Self, DiagnosticError> {
        let capacity = NonZeroUsize::new(capacity).ok_or_else(|| {
            DiagnosticError::ValidationError(
                "Telemetry history capacity must be greater than zero".into(),
            )
        })?;
        Ok(Self {
            adapter,
            capacity,
            // Allocate on demand instead of eagerly reserving untrusted capacity.
            history: VecDeque::new(),
            started_at: Instant::now(),
            last_sequence: 0,
        })
    }

    /// Samples exactly once and returns an owned snapshot also retained in history.
    /// RAM failures are data in the snapshot; only collector failures use Err.
    pub fn sample(&mut self) -> Result<TelemetrySnapshot, DiagnosticError> {
        let sequence = self.last_sequence.checked_add(1).ok_or_else(|| {
            DiagnosticError::StorageError("Telemetry sequence number exhausted".into())
        })?;
        let ram = self.adapter.ram_info();
        let snapshot = TelemetrySnapshot {
            sequence,
            captured_at: SystemTime::now().into(),
            elapsed: self.started_at.elapsed(),
            ram,
        };
        if self.history.len() == self.capacity.get() {
            self.history.pop_front();
        }
        self.history.push_back(snapshot.clone());
        self.last_sequence = sequence;
        Ok(snapshot)
    }

    pub fn capacity(&self) -> usize {
        self.capacity.get()
    }

    /// Read-only access prevents callers from bypassing retention or ordering.
    pub fn history(&self) -> &VecDeque<TelemetrySnapshot> {
        &self.history
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use u_monitor_core::{Availability, PlatformFeature};

    struct EmptyAdapter;
    impl PlatformAdapter for EmptyAdapter {
        fn name(&self) -> &'static str {
            "Empty test adapter"
        }
        fn check_availability(&self, _: PlatformFeature) -> Availability {
            Availability::NotImplemented
        }
    }

    #[test]
    fn sequence_exhaustion_does_not_wrap_or_modify_history() {
        let mut collector = TelemetryCollector::new(EmptyAdapter, 1).expect("valid capacity");
        collector.last_sequence = u64::MAX - 1;
        let last = collector.sample().expect("last representable sequence");
        assert_eq!(last.sequence, u64::MAX);
        assert!(matches!(
            collector.sample(),
            Err(DiagnosticError::StorageError(_))
        ));
        assert_eq!(collector.history().len(), 1);
        assert_eq!(collector.history().front(), Some(&last));
    }
}

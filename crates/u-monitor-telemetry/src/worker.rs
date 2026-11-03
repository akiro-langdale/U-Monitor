//! One owner for the collector; a bounded, nonblocking notification channel.
use crate::{TelemetryCollector, TelemetrySnapshot};
use std::{
    collections::VecDeque,
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicU64, Ordering},
        mpsc::{self, Receiver, RecvTimeoutError, SyncSender, TryRecvError, TrySendError},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
use u_monitor_core::{DiagnosticError, PlatformAdapter};

#[derive(Debug, Clone, Copy)]
pub struct WorkerConfig {
    /// Pause after a completed sample; this is not a hard real-time deadline.
    pub interval: Duration,
    pub history_capacity: usize,
    pub channel_capacity: usize,
}

#[derive(Debug)]
pub struct WorkerReport {
    pub history: VecDeque<TelemetrySnapshot>,
    pub samples_collected: u64,
    /// Only notifications rejected by a full channel, not history evictions.
    pub dropped_notifications: u64,
    /// Adapter errors remain in snapshots; this describes the collector/worker.
    pub result: Result<(), DiagnosticError>,
}

struct Schedule {
    interval: Duration,
    next_sample: Instant,
    stopping: bool,
}

struct Control {
    schedule: Mutex<Schedule>,
    changed: Condvar,
}

fn failure(message: &str) -> DiagnosticError {
    DiagnosticError::StorageError(message.into())
}

fn deadline(interval: Duration) -> Result<Instant, DiagnosticError> {
    if interval.is_zero() {
        return Err(DiagnosticError::ValidationError(
            "Sampling interval must be greater than zero".into(),
        ));
    }
    Instant::now().checked_add(interval).ok_or_else(|| {
        DiagnosticError::ValidationError(
            "Sampling interval is too large for the monotonic clock".into(),
        )
    })
}

impl Control {
    fn stop(&self) {
        // Recover only to publish shutdown. Normal operations report poisoning.
        let mut state = match self.schedule.lock() {
            Ok(state) => state,
            Err(poisoned) => poisoned.into_inner(),
        };
        state.stopping = true;
        self.changed.notify_all();
    }

    fn wait_due(&self) -> Result<bool, DiagnosticError> {
        let mut state = self
            .schedule
            .lock()
            .map_err(|_| failure("Worker schedule mutex poisoned"))?;
        loop {
            if state.stopping {
                return Ok(false);
            }
            let now = Instant::now();
            if now >= state.next_sample {
                return Ok(true);
            }
            let remaining = state.next_sample.saturating_duration_since(now);
            let (updated, _) = self
                .changed
                .wait_timeout(state, remaining)
                .map_err(|_| failure("Worker schedule mutex poisoned while waiting"))?;
            state = updated;
            // Re-check the predicate after commands, timeouts and spurious wakes.
        }
    }

    fn schedule_next(&self) -> Result<(), DiagnosticError> {
        let mut state = self
            .schedule
            .lock()
            .map_err(|_| failure("Worker schedule mutex poisoned"))?;
        state.next_sample = deadline(state.interval)?;
        Ok(())
    }
}

/// Dropping the handle requests shutdown and joins the worker.
/// stop/Drop wait for any already selected or in-flight adapter call to finish.
pub struct TelemetryWorker {
    control: Arc<Control>,
    receiver: Receiver<TelemetrySnapshot>,
    dropped: Arc<AtomicU64>,
    join: Option<JoinHandle<WorkerReport>>,
}

impl TelemetryWorker {
    pub fn start<A: PlatformAdapter + Send + 'static>(
        adapter: A,
        config: WorkerConfig,
    ) -> Result<Self, DiagnosticError> {
        deadline(config.interval)?;
        if config.channel_capacity == 0 {
            return Err(DiagnosticError::ValidationError(
                "Snapshot channel capacity must be greater than zero".into(),
            ));
        }
        let collector = TelemetryCollector::new(adapter, config.history_capacity)?;
        let control = Arc::new(Control {
            schedule: Mutex::new(Schedule {
                interval: config.interval,
                next_sample: Instant::now(),
                stopping: false,
            }),
            changed: Condvar::new(),
        });
        let dropped = Arc::new(AtomicU64::new(0));
        let (sender, receiver) = mpsc::sync_channel(config.channel_capacity);
        let worker_control = Arc::clone(&control);
        let worker_dropped = Arc::clone(&dropped);
        let join = thread::Builder::new()
            .name("u-monitor-ram".into())
            .spawn(move || run(collector, sender, worker_control, worker_dropped))
            .map_err(|error| failure(&format!("Failed to spawn telemetry worker: {error}")))?;
        Ok(Self {
            control,
            receiver,
            dropped,
            join: Some(join),
        })
    }

    pub fn recv_timeout(&self, timeout: Duration) -> Result<TelemetrySnapshot, RecvTimeoutError> {
        self.receiver.recv_timeout(timeout)
    }

    pub fn try_recv(&self) -> Result<TelemetrySnapshot, TryRecvError> {
        self.receiver.try_recv()
    }

    /// Resets the next wait from now. If an adapter call is active, the new pause
    /// is applied after it completes. Success means accepted, not yet sampled.
    pub fn set_interval(&self, interval: Duration) -> Result<(), DiagnosticError> {
        let next_sample = deadline(interval)?;
        let mut state = self
            .control
            .schedule
            .lock()
            .map_err(|_| failure("Worker schedule mutex poisoned"))?;
        if state.stopping || self.join.as_ref().is_none_or(|join| join.is_finished()) {
            return Err(failure("Cannot change interval of a stopped worker"));
        }
        state.interval = interval;
        state.next_sample = next_sample;
        self.control.changed.notify_all();
        Ok(())
    }

    pub fn dropped_notifications(&self) -> u64 {
        self.dropped.load(Ordering::Relaxed)
    }

    pub fn request_stop(&self) {
        self.control.stop();
    }

    /// Joins the worker and returns retained history, including samples whose
    /// notifications were dropped. A worker panic returns Err without history.
    pub fn stop(mut self) -> Result<WorkerReport, DiagnosticError> {
        self.request_stop();
        let join = self
            .join
            .take()
            .ok_or_else(|| failure("Telemetry worker already joined"))?;
        join.join()
            .map_err(|_| failure("Telemetry worker panicked; final history is unavailable"))
    }
}

impl Drop for TelemetryWorker {
    fn drop(&mut self) {
        self.request_stop();
        if let Some(join) = self.join.take() {
            // Drop cannot return errors. Call stop explicitly to inspect outcome.
            let _ = join.join();
        }
    }
}

fn run<A: PlatformAdapter>(
    mut collector: TelemetryCollector<A>,
    sender: SyncSender<TelemetrySnapshot>,
    control: Arc<Control>,
    dropped: Arc<AtomicU64>,
) -> WorkerReport {
    let result = (|| {
        while control.wait_due()? {
            let snapshot = collector.sample()?;
            match sender.try_send(snapshot) {
                Ok(()) => {}
                Err(TrySendError::Full(_)) => {
                    let _ = dropped.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |count| {
                        Some(count.saturating_add(1))
                    });
                }
                Err(TrySendError::Disconnected(_)) => break,
            }
            control.schedule_next()?;
        }
        Ok(())
    })();
    WorkerReport {
        samples_collected: collector.last_sequence,
        history: collector.history,
        dropped_notifications: dropped.load(Ordering::Relaxed),
        result,
    }
}

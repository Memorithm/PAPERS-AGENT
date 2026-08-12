//! Signaling pattern — event system (observer), coordination primitives,
//! condition variables for the PAPERS evolution pipeline.

use parking_lot::{Condvar, Mutex};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

// ── Event system (Observer pattern) ────────────────────────────

/// An event emitted by the PAPERS pipeline.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum PipelineEvent {
    ExtractionStarted { source: String },
    ExtractionCompleted { doc_id: String },
    AnalysisStarted { doc_id: String },
    AnalysisCompleted { doc_id: String, score: i64 },
    EvolutionRoundStarted { round: usize },
    EvolutionRoundCompleted { round: usize, best_score: i64 },
    CandidateEvaluated { candidate_id: String, score: i64 },
    ErrorOccurred { context: String, message: String },
    PipelineCompleted,
    Shutdown,
}

/// Trait for event listeners (observers).
pub trait EventListener: Send + Sync {
    fn on_event(&self, _event: &PipelineEvent) {}
}

/// Thread-safe event bus using Observer pattern.
pub struct EventBus {
    listeners: Mutex<HashMap<String, Vec<Arc<dyn EventListener>>>>,
    event_history: Mutex<Vec<(PipelineEvent, Duration)>>,
}

impl EventBus {
    pub fn new() -> Self {
        Self {
            listeners: Mutex::new(HashMap::new()),
            event_history: Mutex::new(Vec::with_capacity(1000)),
        }
    }

    /// Register a listener for a specific event type or all events ("*").
    pub fn subscribe(&self, event_type: &str, listener: Arc<dyn EventListener>) {
        self.listeners
            .lock()
            .entry(event_type.to_string())
            .or_default()
            .push(listener);
    }

    /// Emit an event to all matching listeners.
    pub fn emit(&self, event: PipelineEvent) {
        let key = event_key(&event);
        let listeners = self.listeners.lock();
        let start = std::time::Instant::now();

        // Notify specific listeners
        if let Some(specific) = listeners.get(&key) {
            for listener in specific {
                listener.on_event(&event);
            }
        }

        // Notify wildcard listeners
        if let Some(wildcard) = listeners.get("*") {
            for listener in wildcard {
                listener.on_event(&event);
            }
        }

        // Record event in history
        self.event_history.lock().push((event, start.elapsed()));
    }

    /// Get recent event history.
    pub fn recent_events(&self, n: usize) -> Vec<PipelineEvent> {
        self.event_history
            .lock()
            .iter()
            .rev()
            .take(n)
            .map(|(e, _)| e.clone())
            .collect()
    }

    /// Count events by type.
    pub fn event_counts(&self) -> HashMap<String, usize> {
        let mut counts = HashMap::new();
        for (event, _) in self.event_history.lock().iter() {
            *counts.entry(event_key(event)).or_default() += 1;
        }
        counts
    }
}

fn event_key(event: &PipelineEvent) -> String {
    match event {
        PipelineEvent::ExtractionStarted { .. } => "ExtractionStarted".into(),
        PipelineEvent::ExtractionCompleted { .. } => "ExtractionCompleted".into(),
        PipelineEvent::AnalysisStarted { .. } => "AnalysisStarted".into(),
        PipelineEvent::AnalysisCompleted { .. } => "AnalysisCompleted".into(),
        PipelineEvent::EvolutionRoundStarted { .. } => "EvolutionRoundStarted".into(),
        PipelineEvent::EvolutionRoundCompleted { .. } => "EvolutionRoundCompleted".into(),
        PipelineEvent::CandidateEvaluated { .. } => "CandidateEvaluated".into(),
        PipelineEvent::ErrorOccurred { .. } => "ErrorOccurred".into(),
        PipelineEvent::PipelineCompleted => "PipelineCompleted".into(),
        PipelineEvent::Shutdown => "Shutdown".into(),
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

// ── Signaling channels ─────────────────────────────────────────

/// A signal that can be waited on by multiple threads.
pub struct Signal {
    condvar: Condvar,
    mutex: Mutex<bool>,
}

impl Signal {
    pub fn new() -> Self {
        Self {
            condvar: Condvar::new(),
            mutex: Mutex::new(false),
        }
    }

    /// Wait until the signal is set.
    pub fn wait(&self) {
        let mut guard = self.mutex.lock();
        while !*guard {
            self.condvar.wait(&mut guard);
        }
    }

    /// Wait with a timeout. Returns true if signaled, false on timeout.
    pub fn wait_timeout(&self, timeout: Duration) -> bool {
        let mut guard = self.mutex.lock();
        if *guard {
            return true;
        }
        let mut remaining = timeout;
        loop {
            let start = std::time::Instant::now();
            let result = self.condvar.wait_for(&mut guard, remaining);
            if *guard {
                return true;
            }
            let elapsed = start.elapsed();
            if result.timed_out() || elapsed >= remaining {
                return false;
            }
            remaining = remaining.saturating_sub(elapsed);
        }
    }

    /// Set the signal and wake all waiters.
    pub fn signal(&self) {
        let mut guard = self.mutex.lock();
        *guard = true;
        self.condvar.notify_all();
    }

    /// Reset the signal for reuse.
    pub fn reset(&self) {
        *self.mutex.lock() = false;
    }
}

impl Default for Signal {
    fn default() -> Self {
        Self::new()
    }
}

// ── Coordination primitives ────────────────────────────────────

/// A barrier that waits for N threads to arrive.
pub struct CoordinationBarrier {
    count: Mutex<usize>,
    target: usize,
    condvar: Condvar,
}

impl CoordinationBarrier {
    pub fn new(target: usize) -> Self {
        Self {
            count: Mutex::new(0),
            target,
            condvar: Condvar::new(),
        }
    }

    /// Wait until all threads have arrived.
    pub fn wait(&self) {
        let mut count = self.count.lock();
        *count += 1;
        if *count >= self.target {
            self.condvar.notify_all();
        } else {
            self.condvar.wait(&mut count);
        }
    }
}

/// A latch that counts down and fires when zero is reached.
pub struct CountDownLatch {
    count: Mutex<usize>,
    condvar: Condvar,
}

impl CountDownLatch {
    pub fn new(count: usize) -> Self {
        Self {
            count: Mutex::new(count),
            condvar: Condvar::new(),
        }
    }

    /// Decrement the count. Returns true if all arrived.
    pub fn count_down(&self) -> bool {
        let mut count = self.count.lock();
        if *count > 0 {
            *count -= 1;
        }
        if *count == 0 {
            self.condvar.notify_all();
            true
        } else {
            false
        }
    }

    /// Wait until the latch reaches zero.
    pub fn wait(&self) {
        let mut count = self.count.lock();
        while *count > 0 {
            self.condvar.wait(&mut count);
        }
    }
}

// ── Condition variables ────────────────────────────────────────

/// A rendezvous point for two threads to exchange data.
pub struct Rendezvous<T> {
    value: Mutex<Option<T>>,
    ready: Condvar,
    done: Condvar,
}

impl<T: Clone + Send> Rendezvous<T> {
    pub fn new() -> Self {
        Self {
            value: Mutex::new(None),
            ready: Condvar::new(),
            done: Condvar::new(),
        }
    }

    /// Producer: provide a value and wait for consumer.
    pub fn provide(&self, val: T) {
        let mut guard = self.value.lock();
        *guard = Some(val);
        self.ready.notify_one();
        self.done.wait(&mut guard);
    }

    /// Consumer: wait for a value and consume it.
    pub fn consume(&self) -> T {
        let mut guard = self.value.lock();
        while guard.is_none() {
            self.ready.wait(&mut guard);
        }
        let val = guard.take().unwrap();
        self.done.notify_one();
        val
    }
}

impl<T: Clone + Send> Default for Rendezvous<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn test_signal() {
        let signal = Arc::new(Signal::new());
        let s_clone = signal.clone();
        let handle = thread::spawn(move || {
            s_clone.wait();
        });
        thread::sleep(Duration::from_millis(50));
        signal.signal();
        handle.join().unwrap();
    }

    #[test]
    fn test_countdown_latch() {
        let latch = Arc::new(CountDownLatch::new(3));
        let mut handles = Vec::new();
        for _ in 0..3 {
            let latch = latch.clone();
            handles.push(thread::spawn(move || {
                latch.count_down();
            }));
        }
        for h in handles {
            h.join().unwrap();
        }
    }

    #[test]
    fn test_rendezvous() {
        let rdv = Arc::new(Rendezvous::new());
        let rdv_clone = rdv.clone();
        let producer = thread::spawn(move || {
            rdv_clone.provide(42);
        });
        let consumer = thread::spawn(move || {
            assert_eq!(rdv.consume(), 42);
        });
        producer.join().unwrap();
        consumer.join().unwrap();
    }

    #[test]
    fn test_event_bus() {
        struct TestListener;
        impl EventListener for TestListener {
            fn on_event(&self, event: &PipelineEvent) {
                assert!(matches!(event, PipelineEvent::PipelineCompleted));
            }
        }

        let bus = EventBus::new();
        bus.subscribe("*", Arc::new(TestListener));
        bus.emit(PipelineEvent::PipelineCompleted);
    }

    #[test]
    fn test_coordination_barrier_multiple_threads() {
        let barrier = Arc::new(CoordinationBarrier::new(3));
        let mut handles = Vec::new();
        for i in 0..3 {
            let b = barrier.clone();
            handles.push(thread::spawn(move || {
                b.wait();
                i
            }));
        }
        let results: Vec<usize> = handles.into_iter().map(|h| h.join().unwrap()).collect();
        assert_eq!(results.len(), 3);
    }

    #[test]
    fn test_signal_wait_timeout_and_reset() {
        let signal = Signal::new();
        assert!(!signal.wait_timeout(Duration::from_millis(10)));
        signal.signal();
        assert!(signal.wait_timeout(Duration::from_millis(10)));
        signal.reset();
        assert!(!signal.wait_timeout(Duration::from_millis(10)));
    }

    #[test]
    fn test_event_bus_specific_subscription() {
        use std::sync::Mutex as StdMutex;

        struct CountListener {
            count: StdMutex<usize>,
        }
        impl EventListener for CountListener {
            fn on_event(&self, _event: &PipelineEvent) {
                *self.count.lock().unwrap() += 1;
            }
        }

        let bus = EventBus::new();
        let listener = Arc::new(CountListener {
            count: StdMutex::new(0),
        });
        bus.subscribe("ExtractionStarted", listener.clone());

        bus.emit(PipelineEvent::ExtractionStarted {
            source: "test".into(),
        });
        assert_eq!(*listener.count.lock().unwrap(), 1);

        bus.emit(PipelineEvent::PipelineCompleted);
        assert_eq!(*listener.count.lock().unwrap(), 1);
    }

    #[test]
    fn test_event_bus_recent_events_and_counts() {
        let bus = EventBus::new();
        bus.emit(PipelineEvent::ExtractionStarted { source: "a".into() });
        bus.emit(PipelineEvent::ExtractionCompleted { doc_id: "a".into() });
        bus.emit(PipelineEvent::Shutdown);

        let recent = bus.recent_events(2);
        assert_eq!(recent.len(), 2);
        assert!(matches!(recent[0], PipelineEvent::Shutdown));

        let counts = bus.event_counts();
        assert_eq!(counts.get("ExtractionStarted"), Some(&1));
        assert_eq!(counts.get("ExtractionCompleted"), Some(&1));
        assert_eq!(counts.get("Shutdown"), Some(&1));
    }
}

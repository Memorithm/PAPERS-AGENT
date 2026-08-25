//! Queue pattern — priority-based work queue, result queue, channel abstractions,
//! and backpressure mechanism for the PAPERS pipeline.

use parking_lot::Mutex;
use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::time::{Duration, Instant};

// ── Priority-based work queue ──────────────────────────────────

/// A work item with priority and metadata.
#[derive(Debug, Clone)]
pub struct WorkItem<T> {
    pub id: String,
    pub payload: T,
    pub priority: i32,
    pub enqueued_at: Instant,
    pub timeout: Duration,
}

impl<T> WorkItem<T> {
    pub fn new(id: impl Into<String>, payload: T, priority: i32, timeout: Duration) -> Self {
        Self {
            id: id.into(),
            payload,
            priority,
            enqueued_at: Instant::now(),
            timeout,
        }
    }

    /// Whether the item has been waiting too long.
    pub fn is_stale(&self) -> bool {
        self.enqueued_at.elapsed() >= self.timeout
    }
}

impl<T> Ord for WorkItem<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        // Higher priority first, then older first
        self.priority
            .cmp(&other.priority)
            .then_with(|| other.enqueued_at.cmp(&self.enqueued_at))
    }
}

impl<T> PartialOrd for WorkItem<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<T> Eq for WorkItem<T> {}
impl<T> PartialEq for WorkItem<T> {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id && self.priority == other.priority
    }
}

/// Thread-safe priority work queue with backpressure.
pub struct WorkQueue<T> {
    heap: Mutex<BinaryHeap<WorkItem<T>>>,
    max_size: usize,
    #[allow(dead_code)]
    max_wait: Duration,
    insert_count: Mutex<u64>,
    pop_count: Mutex<u64>,
}

impl<T> WorkQueue<T> {
    pub fn new(max_size: usize, max_wait: Duration) -> Self {
        Self {
            heap: Mutex::new(BinaryHeap::new()),
            max_size,
            max_wait,
            insert_count: Mutex::new(0),
            pop_count: Mutex::new(0),
        }
    }

    /// Push a work item. Returns Err if queue is full (backpressure).
    pub fn push(&self, item: WorkItem<T>) -> Result<(), String> {
        let mut heap = self.heap.lock();
        if heap.len() >= self.max_size {
            // Try to evict stale items to make room
            let stale_count = heap.iter().filter(|i| i.is_stale()).count();
            if stale_count == 0 {
                return Err("Work queue full — apply backpressure".into());
            }
            // Drain expired items and retry
            *heap = heap.drain().filter(|i| !i.is_stale()).collect();
            if heap.len() >= self.max_size {
                return Err("Work queue still full after stale eviction".into());
            }
        }
        heap.push(item);
        *self.insert_count.lock() += 1;
        Ok(())
    }

    /// Pop the highest priority item. Returns None if empty.
    pub fn pop(&self) -> Option<WorkItem<T>> {
        let mut heap = self.heap.lock();
        let item = heap.pop()?;
        *self.pop_count.lock() += 1;
        Some(item)
    }

    /// Prune stale items and return them.
    pub fn drain_stale(&self) -> Vec<WorkItem<T>> {
        let mut heap = self.heap.lock();
        let (stale, fresh): (Vec<_>, Vec<_>) = heap.drain().partition(|i| i.is_stale());
        *heap = fresh.into();
        stale
    }

    pub fn len(&self) -> usize {
        self.heap.lock().len()
    }

    pub fn is_empty(&self) -> bool {
        self.heap.lock().is_empty()
    }

    /// Total items inserted (for metrics).
    pub fn stats(&self) -> (u64, u64) {
        (*self.insert_count.lock(), *self.pop_count.lock())
    }
}

// ── Result queue ────────────────────────────────────────────────

/// A completed work result with timing and status.    
#[derive(Debug, Clone)]
pub struct WorkResult<T> {
    pub work_id: String,
    pub result: Result<T, String>,
    pub started_at: Instant,
    pub completed_at: Instant,
}

impl<T> WorkResult<T> {
    pub fn success(work_id: impl Into<String>, value: T, started_at: Instant) -> Self {
        Self {
            work_id: work_id.into(),
            result: Ok(value),
            started_at,
            completed_at: Instant::now(),
        }
    }

    pub fn failure(work_id: impl Into<String>, error: String, started_at: Instant) -> Self {
        Self {
            work_id: work_id.into(),
            result: Err(error),
            started_at,
            completed_at: Instant::now(),
        }
    }

    pub fn elapsed(&self) -> Duration {
        self.completed_at - self.started_at
    }
}

/// Thread-safe result queue.
pub struct ResultQueue<T> {
    results: Mutex<Vec<WorkResult<T>>>,
    max_size: usize,
}

impl<T: Clone> ResultQueue<T> {
    pub fn new(max_size: usize) -> Self {
        Self {
            results: Mutex::new(Vec::with_capacity(max_size)),
            max_size,
        }
    }

    /// Push a result. Oldest results are evicted if at capacity.
    pub fn push(&self, result: WorkResult<T>) {
        let mut results = self.results.lock();
        if results.len() >= self.max_size {
            results.remove(0);
        }
        results.push(result);
    }

    /// Get recent results.
    pub fn recent(&self, n: usize) -> Vec<WorkResult<T>> {
        let results = self.results.lock();
        results.iter().rev().take(n).cloned().collect()
    }

    /// Count successes and failures.
    pub fn summary(&self) -> (usize, usize) {
        let results = self.results.lock();
        let successes = results.iter().filter(|r| r.result.is_ok()).count();
        let failures = results.len() - successes;
        (successes, failures)
    }

    pub fn len(&self) -> usize {
        self.results.lock().len()
    }

    pub fn is_empty(&self) -> bool {
        self.results.lock().is_empty()
    }
}

// ── Channel abstraction ────────────────────────────────────────

/// Multi-producer, multi-consumer channel for work items.
pub struct Channel<T> {
    tx: tokio::sync::mpsc::Sender<T>,
    rx: tokio::sync::Mutex<Option<tokio::sync::mpsc::Receiver<T>>>,
}

impl<T> Channel<T> {
    pub fn new(buffer: usize) -> Self {
        let (tx, rx) = tokio::sync::mpsc::channel(buffer);
        Self {
            tx,
            rx: tokio::sync::Mutex::new(Some(rx)),
        }
    }

    /// Send a message (non-blocking, returns full error if buffer exhausted).
    pub async fn send(&self, msg: T) -> Result<(), tokio::sync::mpsc::error::SendError<T>> {
        self.tx.send(msg).await
    }

    /// Try to send without waiting.
    pub fn send_blocking(&self, msg: T) -> Result<(), tokio::sync::mpsc::error::TrySendError<T>> {
        self.tx.try_send(msg)
    }

    /// Receive a message.
    pub async fn recv(&self) -> Option<T> {
        let mut guard = self.rx.lock().await;
        match &mut *guard {
            Some(rx) => rx.recv().await,
            None => None,
        }
    }
}

// ── Backpressure mechanism ──────────────────────────────────────

/// Backpressure controller that governs flow based on queue sizes.
pub struct BackpressureController {
    high_watermark: usize,
    low_watermark: usize,
    current_pressure: Mutex<PressureState>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PressureState {
    Normal,
    Warning,
    Critical,
}

impl BackpressureController {
    pub fn new(high_watermark: usize, low_watermark: usize) -> Self {
        Self {
            high_watermark,
            low_watermark,
            current_pressure: Mutex::new(PressureState::Normal),
        }
    }

    /// Update pressure based on current queue size.
    pub fn update(&self, queue_size: usize) -> PressureState {
        let state = if queue_size >= self.high_watermark {
            PressureState::Critical
        } else if queue_size >= self.low_watermark {
            PressureState::Warning
        } else {
            PressureState::Normal
        };
        *self.current_pressure.lock() = state;
        state
    }

    /// Whether the system can accept more work.
    pub fn can_accept(&self, queue_size: usize) -> bool {
        self.update(queue_size);
        let state = *self.current_pressure.lock();
        state != PressureState::Critical
    }

    pub fn current_state(&self) -> PressureState {
        *self.current_pressure.lock()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_work_queue_priority() {
        let q = WorkQueue::<String>::new(10, Duration::from_secs(60));
        q.push(WorkItem::new("a", "low".into(), 1, Duration::from_secs(60)))
            .unwrap();
        q.push(WorkItem::new(
            "b",
            "high".into(),
            10,
            Duration::from_secs(60),
        ))
        .unwrap();
        q.push(WorkItem::new(
            "c",
            "medium".into(),
            5,
            Duration::from_secs(60),
        ))
        .unwrap();

        assert_eq!(q.pop().unwrap().payload, "high");
        assert_eq!(q.pop().unwrap().payload, "medium");
        assert_eq!(q.pop().unwrap().payload, "low");
    }

    #[test]
    fn test_backpressure() {
        let ctrl = BackpressureController::new(8, 4);
        assert!(ctrl.can_accept(0));
        assert!(ctrl.can_accept(3));
        assert!(ctrl.can_accept(5));
        assert!(!ctrl.can_accept(10));
        assert_eq!(ctrl.current_state(), PressureState::Critical);
    }

    #[test]
    fn test_result_queue() {
        let rq = ResultQueue::<i32>::new(5);
        let now = Instant::now();
        rq.push(WorkResult::success("1", 42, now));
        rq.push(WorkResult::failure("2", "boom".into(), now));
        let (ok, fail) = rq.summary();
        assert_eq!(ok, 1);
        assert_eq!(fail, 1);
    }
}

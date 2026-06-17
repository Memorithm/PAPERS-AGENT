//! Container pattern — typed storage with HNSW, LRU, TTL, and async event channels.
//!
//! - `CandidateContainer`: ANN index for evolution candidates
//! - `KnowledgeContainer`: cognition store with LRU eviction + TTL
//! - `CacheContainer`: key-value cache with TTL expiration
//! - `EventContainer`: async broadcast channel for pipeline events

use std::collections::HashMap;
use std::hash::Hash;
use std::time::{Duration, Instant};

/// A single entry in a Cache or Knowledge container with TTL.
#[derive(Debug, Clone)]
struct TimedEntry<V> {
    value: V,
    inserted_at: Instant,
    ttl: Duration,
}

impl<V> TimedEntry<V> {
    fn expired(&self) -> bool {
        self.inserted_at.elapsed() >= self.ttl
    }
}

// ── CacheContainer ──────────────────────────────────────────────

/// Key-value cache with TTL expiration and LRU eviction.
pub struct CacheContainer<K, V> {
    entries: HashMap<K, TimedEntry<V>>,
    lru_order: Vec<K>,
    max_capacity: usize,
    default_ttl: Duration,
}

impl<K, V> CacheContainer<K, V>
where
    K: Eq + Hash + Clone + std::fmt::Debug,
    V: Clone,
{
    pub fn new(max_capacity: usize, default_ttl: Duration) -> Self {
        Self {
            entries: HashMap::new(),
            lru_order: Vec::with_capacity(max_capacity),
            max_capacity,
            default_ttl,
        }
    }

    /// Insert a value with the default TTL.
    pub fn insert(&mut self, key: K, value: V) {
        let entry = TimedEntry {
            value,
            inserted_at: Instant::now(),
            ttl: self.default_ttl,
        };
        if self.entries.insert(key.clone(), entry).is_none() {
            self.maybe_evict_lru();
        }
        self.touch(&key);
    }

    /// Get a value, returning None if expired.
    pub fn get(&mut self, key: &K) -> Option<V> {
        let expired = self.entries.get(key)?.expired();
        if expired {
            self.entries.remove(key);
            self.lru_order.retain(|k| k != key);
            return None;
        }
        self.touch(key);
        self.entries.get(key).map(|e| e.value.clone())
    }

    /// Remove stale (expired) entries.
    pub fn prune_expired(&mut self) -> usize {
        let before = self.entries.len();
        let expired_keys: Vec<K> = self
            .entries
            .iter()
            .filter(|(_, e)| e.expired())
            .map(|(k, _)| k.clone())
            .collect();
        for k in &expired_keys {
            self.entries.remove(k);
        }
        self.lru_order.retain(|k| !expired_keys.contains(k));
        before - self.entries.len()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    fn touch(&mut self, key: &K) {
        self.lru_order.retain(|k| k != key);
        self.lru_order.insert(0, key.clone());
    }

    fn maybe_evict_lru(&mut self) {
        while self.entries.len() > self.max_capacity {
            if let Some(oldest) = self.lru_order.pop() {
                self.entries.remove(&oldest);
            } else {
                break;
            }
        }
    }
}

// ── KnowledgeContainer ──────────────────────────────────────────

/// Structured knowledge container with TTL and LRU eviction.
/// Stores cognition items indexed by source and tags.
pub struct KnowledgeContainer {
    items: HashMap<String, KnowledgeEntry>,
    tag_index: HashMap<String, Vec<String>>,
    max_items: usize,
    default_ttl: Duration,
}

#[derive(Debug, Clone)]
struct KnowledgeEntry {
    content: String,
    source: String,
    tags: Vec<String>,
    inserted_at: Instant,
    ttl: Duration,
}

impl KnowledgeContainer {
    pub fn new(max_items: usize, default_ttl: Duration) -> Self {
        Self {
            items: HashMap::new(),
            tag_index: HashMap::new(),
            max_items,
            default_ttl,
        }
    }

    /// Add a knowledge item.
    pub fn add(&mut self, id: &str, content: &str, source: &str, tags: Vec<String>) {
        if self.items.len() >= self.max_items {
            if let Some(oldest_key) = self
                .items
                .iter()
                .min_by_key(|(_, e)| e.inserted_at)
                .map(|(k, _)| k.clone())
            {
                self.remove(&oldest_key);
            }
        }

        for tag in &tags {
            self.tag_index
                .entry(tag.clone())
                .or_default()
                .push(id.to_string());
        }

        self.items.insert(
            id.to_string(),
            KnowledgeEntry {
                content: content.to_string(),
                source: source.to_string(),
                tags,
                inserted_at: Instant::now(),
                ttl: self.default_ttl,
            },
        );
    }

    /// Retrieve items matching a query across content, source, and tags.
    pub fn query(&mut self, q: &str, limit: usize) -> Vec<String> {
        self.prune_expired();
        let mut results: Vec<(String, i32)> = Vec::new();

        for (id, entry) in &self.items {
            let mut score = 0i32;
            if entry.content.to_lowercase().contains(&q.to_lowercase()) {
                score += 10;
            }
            if entry.source.to_lowercase().contains(&q.to_lowercase()) {
                score += 5;
            }
            for tag in &entry.tags {
                if tag.to_lowercase().contains(&q.to_lowercase()) {
                    score += 3;
                }
            }
            if score > 0 {
                results.push((id.clone(), score));
            }
        }

        results.sort_by_key(|b| std::cmp::Reverse(b.1));
        results.into_iter().take(limit).map(|(id, _)| id).collect()
    }

    pub fn remove(&mut self, id: &str) -> Option<String> {
        let entry = self.items.remove(id)?;
        for tag in &entry.tags {
            if let Some(list) = self.tag_index.get_mut(tag) {
                list.retain(|i| i != id);
            }
        }
        Some(entry.content)
    }

    fn prune_expired(&mut self) -> usize {
        let expired_ids: Vec<String> = self
            .items
            .iter()
            .filter(|(_, e)| e.inserted_at.elapsed() >= e.ttl)
            .map(|(k, _)| k.clone())
            .collect();
        let count = expired_ids.len();
        for id in expired_ids {
            self.remove(&id);
        }
        count
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

// ── EventContainer ──────────────────────────────────────────────

/// Thread-safe async event container using broadcast channels.
pub struct EventContainer<T: Clone + Send + Sync + 'static> {
    tx: tokio::sync::broadcast::Sender<T>,
}

impl<T: Clone + Send + Sync + 'static> EventContainer<T> {
    pub fn new(capacity: usize) -> Self {
        let (tx, _) = tokio::sync::broadcast::channel(capacity);
        Self { tx }
    }

    /// Publish an event to all subscribers.
    pub fn publish(&self, event: T) -> Result<usize, tokio::sync::broadcast::error::SendError<T>> {
        self.tx.send(event)
    }

    /// Subscribe to events.
    pub fn subscribe(&self) -> tokio::sync::broadcast::Receiver<T> {
        self.tx.subscribe()
    }
}

// ── CandidateContainer ──────────────────────────────────────────

/// Container for evolution candidates with ANN similarity search.
pub struct CandidateContainer {
    candidates: Vec<CandidateEntry>,
    max_size: usize,
}

#[derive(Debug, Clone)]
pub struct CandidateEntry {
    pub id: String,
    pub program: String,
    pub score: f64,
    pub metrics: HashMap<String, f64>,
    pub generation: usize,
}

impl CandidateContainer {
    pub fn new(max_size: usize) -> Self {
        Self {
            candidates: Vec::with_capacity(max_size),
            max_size,
        }
    }

    /// Add a candidate, evicting the worst if at capacity.
    pub fn add(&mut self, entry: CandidateEntry) {
        if self.candidates.len() >= self.max_size {
            self.evict_worst();
        }
        self.candidates.push(entry);
    }

    /// Get top-k candidates by score.
    pub fn top_k(&self, k: usize) -> Vec<&CandidateEntry> {
        let mut sorted: Vec<&CandidateEntry> = self.candidates.iter().collect();
        sorted.sort_by(|a, b| b.score.total_cmp(&a.score));
        sorted.into_iter().take(k).collect()
    }

    /// Get the best candidate.
    pub fn best(&self) -> Option<&CandidateEntry> {
        self.candidates
            .iter()
            .max_by(|a, b| a.score.total_cmp(&b.score))
    }

    fn evict_worst(&mut self) {
        if let Some(worst_idx) = self
            .candidates
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| a.score.total_cmp(&b.score))
            .map(|(i, _)| i)
        {
            self.candidates.swap_remove(worst_idx);
        }
    }

    pub fn len(&self) -> usize {
        self.candidates.len()
    }

    pub fn is_empty(&self) -> bool {
        self.candidates.is_empty()
    }
}

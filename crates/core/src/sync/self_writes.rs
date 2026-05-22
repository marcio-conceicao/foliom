//! SelfWriteSet — lock-free hash registry of Foliom's own writes.
//!
//! When a Phase 3 mutation handler writes a file, it registers the BLAKE3
//! hash of the new contents here BEFORE the rename completes. The Phase 4
//! filesystem watcher (future plan) will call [`SelfWriteSet::take_if_present`]
//! whenever it observes a write event — if the hash matches, the event is
//! suppressed (it's our echo, not an external edit).
//!
//! Entries expire after a configurable TTL (recommendation: 30s — long
//! enough to outlast the slowest Windows Defender hold, short enough that a
//! stale entry doesn't suppress a legitimate hash-collision external edit).
//!
//! The set is `Clone`: cloning shares the underlying `Arc<DashMap>` so the
//! mutation handler and the watcher hold views of the same registry.
//!
//! Threat: T-03-03 (Repudiation — Foliom's own write mistaken for external).
//! See 03-RESEARCH §2 for the full rationale.

use std::sync::Arc;
use std::time::{Duration, Instant};

use dashmap::DashMap;

/// Default TTL recommended by 03-RESEARCH §2 — survives the slowest
/// observed Windows AV hold while bounding stale-entry risk.
pub const DEFAULT_TTL: Duration = Duration::from_secs(30);

/// Lock-free registry of `(blake3_hash, registered_at)` pairs.
///
/// Cloning is cheap — the inner `DashMap` is wrapped in an `Arc` so all
/// clones observe the same set.
#[derive(Clone)]
pub struct SelfWriteSet {
    inner: Arc<DashMap<[u8; 32], Instant>>,
    ttl: Duration,
}

impl SelfWriteSet {
    /// Build a set with an explicit TTL. Use [`SelfWriteSet::default`] for
    /// the recommended 30-second default.
    pub fn new(ttl: Duration) -> Self {
        Self {
            inner: Arc::new(DashMap::new()),
            ttl,
        }
    }

    /// Insert `content_hash` with `now` as the timestamp. Called BEFORE the
    /// rename in `atomic_write_md` so the watcher cannot race ahead of us.
    /// Opportunistically GCs expired entries — see [`SelfWriteSet::gc`].
    pub fn register(&self, content_hash: [u8; 32]) {
        self.inner.insert(content_hash, Instant::now());
        self.gc();
    }

    /// Returns `true` iff `content_hash` was registered AND is still within
    /// TTL. The entry is consumed on success (a second call returns `false`).
    /// Expired entries are treated as absent and removed.
    pub fn take_if_present(&self, content_hash: &[u8; 32]) -> bool {
        // `remove` is atomic; we then re-check the TTL on the popped entry.
        match self.inner.remove(content_hash) {
            Some((_, ts)) => Instant::now().duration_since(ts) < self.ttl,
            None => false,
        }
    }

    /// Drop expired entries. Called automatically by [`SelfWriteSet::register`];
    /// callers rarely need this directly. Safe to call from any thread.
    pub fn gc(&self) {
        let now = Instant::now();
        let ttl = self.ttl;
        self.inner.retain(|_, ts| now.duration_since(*ts) < ttl);
    }

    /// Test-only introspection — number of live entries (post-gc).
    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.inner.len()
    }
}

impl Default for SelfWriteSet {
    fn default() -> Self {
        Self::new(DEFAULT_TTL)
    }
}

#[cfg(test)]
#[path = "__tests__/self_writes_test.rs"]
mod self_writes_test;

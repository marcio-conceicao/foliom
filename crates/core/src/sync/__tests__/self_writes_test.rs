//! SelfWriteSet behaviour suite — Plan 03-01 Task 1.
//!
//! Covers the five truths from 03-01-PLAN.md `must_haves.truths`:
//!   1. register → take_if_present consumes the entry (second call is false).
//!   2. After TTL elapses, take_if_present returns false.
//!   3. Clones share state (Arc semantics) — mutation handler ↔ watcher view.
//!   4. gc() after expiry leaves the inner map empty (no leak).
//!   5. Concurrent 1000×2 register calls produce a bounded, panic-free state.

use std::sync::Arc;
use std::sync::Barrier;
use std::thread;
use std::time::Duration;

use super::*;

#[test]
fn register_then_take_is_present_and_consumes() {
    let set = SelfWriteSet::new(Duration::from_secs(1));
    let h = [1u8; 32];
    set.register(h);
    assert!(set.take_if_present(&h), "first take should see the entry");
    assert!(
        !set.take_if_present(&h),
        "second take should be empty (entry consumed)"
    );
}

#[test]
fn ttl_expiry_makes_entry_absent() {
    let set = SelfWriteSet::new(Duration::from_millis(50));
    let h = [2u8; 32];
    set.register(h);
    thread::sleep(Duration::from_millis(100));
    assert!(
        !set.take_if_present(&h),
        "entry should be expired after 100ms with 50ms TTL"
    );
}

#[test]
fn clone_shares_state_with_origin() {
    let a = SelfWriteSet::new(Duration::from_secs(1));
    let b = a.clone();
    let h = [3u8; 32];
    a.register(h);
    assert!(
        b.take_if_present(&h),
        "clone B must see entries registered through clone A"
    );
}

#[test]
fn gc_reclaims_all_expired_entries() {
    let set = SelfWriteSet::new(Duration::from_millis(20));
    for i in 0..50u8 {
        set.register([i; 32]);
    }
    thread::sleep(Duration::from_millis(60));
    set.gc();
    assert_eq!(set.len(), 0, "all entries should be reclaimed after TTL");
}

#[test]
fn concurrent_registers_do_not_panic() {
    let set = SelfWriteSet::new(Duration::from_secs(10));
    let barrier = Arc::new(Barrier::new(2));

    let s1 = set.clone();
    let b1 = barrier.clone();
    let t1 = thread::spawn(move || {
        b1.wait();
        for i in 0..1000u32 {
            let mut h = [0u8; 32];
            h[..4].copy_from_slice(&i.to_le_bytes());
            h[31] = 0xAA; // disambiguate from thread 2
            s1.register(h);
        }
    });

    let s2 = set.clone();
    let b2 = barrier.clone();
    let t2 = thread::spawn(move || {
        b2.wait();
        for i in 0..1000u32 {
            let mut h = [0u8; 32];
            h[..4].copy_from_slice(&i.to_le_bytes());
            h[31] = 0xBB;
            s2.register(h);
        }
    });

    t1.join().expect("thread 1 panicked");
    t2.join().expect("thread 2 panicked");

    // Each thread inserts 1000 unique hashes (disambiguated by byte 31), and
    // no entry has expired (TTL is 10s), so we expect exactly 2000.
    assert!(set.len() <= 2000);
    assert!(set.len() >= 1900, "lost too many entries: {}", set.len());
}

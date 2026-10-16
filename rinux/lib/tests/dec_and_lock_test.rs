// SPDX-License-Identifier: GPL-2.0-or-later
//! Host tests for `atomic_dec_and_lock`. Linux has no KUnit file for it.

use rinux_lib::dec_and_lock::atomic_dec_and_lock;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicUsize, Ordering};

#[test]
fn a_count_above_one_drops_without_the_lock() {
    let count = AtomicI32::new(3);
    let lock = AtomicBool::new(false);
    assert!(!atomic_dec_and_lock(&count, &lock));
    assert_eq!(count.load(Ordering::Relaxed), 2);
    assert!(!lock.load(Ordering::Relaxed));
}

#[test]
fn the_last_reference_returns_with_the_lock_held() {
    let count = AtomicI32::new(1);
    let lock = AtomicBool::new(false);
    assert!(atomic_dec_and_lock(&count, &lock));
    assert_eq!(count.load(Ordering::Relaxed), 0);
    assert!(lock.load(Ordering::Relaxed));
}

#[test]
fn exactly_one_thread_sees_the_last_drop() {
    for _ in 0..50 {
        let count = AtomicI32::new(8);
        let lock = AtomicBool::new(false);
        let winners = AtomicUsize::new(0);
        std::thread::scope(|s| {
            for _ in 0..8 {
                s.spawn(|| {
                    if atomic_dec_and_lock(&count, &lock) {
                        winners.fetch_add(1, Ordering::Relaxed);
                        lock.store(false, Ordering::Release);
                    }
                });
            }
        });
        assert_eq!(winners.load(Ordering::Relaxed), 1);
        assert_eq!(count.load(Ordering::Relaxed), 0);
    }
}

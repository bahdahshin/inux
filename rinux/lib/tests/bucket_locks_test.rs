// SPDX-License-Identifier: GPL-2.0-or-later
//! Host tests for the bucket locks: the sizing rule and mutual exclusion
//! within a bucket. Linux has no KUnit file for `bucket_locks.c`.

use rinux_lib::bucket_locks::{BucketLocks, bucket_locks_size};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

#[test]
fn sizing_follows_the_cpu_multiplier_and_cap() {
    assert_eq!(bucket_locks_size(4, 8, 1024), (32, 31));
    assert_eq!(bucket_locks_size(200, 8, 1024), (512, 511)); // 64 * 8
    assert_eq!(bucket_locks_size(4, 8, 16), (16, 15)); // capped
    assert_eq!(bucket_locks_size(4, 0, 64), (64, 63)); // no multiplier
}

#[test]
fn hashes_select_buckets_by_mask() {
    let storage: Vec<AtomicBool> = (0..8).map(|_| AtomicBool::new(false)).collect();
    let locks = BucketLocks::new(&storage);
    assert_eq!(locks.mask(), 7);
    let _g = locks.lock(8 + 3); // same bucket as 3
    assert!(storage[3].load(Ordering::Relaxed));
    assert!(!storage[4].load(Ordering::Relaxed));
}

#[test]
fn one_bucket_gives_mutual_exclusion() {
    let storage = [AtomicBool::new(false)];
    let locks = BucketLocks::new(&storage);
    let counter = AtomicUsize::new(0);
    std::thread::scope(|s| {
        for _ in 0..4 {
            s.spawn(|| {
                for _ in 0..1000 {
                    let _g = locks.lock(12345);
                    // Read and write separately: only the lock keeps this exact.
                    let v = counter.load(Ordering::Relaxed);
                    counter.store(v + 1, Ordering::Relaxed);
                }
            });
        }
    });
    assert_eq!(counter.load(Ordering::Relaxed), 4000);
}

// SPDX-License-Identifier: GPL-2.0-only
//! A fixed array of spinlocks, indexed by a hash, so that hashed objects can be
//! locked without one lock per object.
//!
//! Ported from `lib/bucket_locks.c`. `__alloc_bucket_spinlocks()` sizes the
//! array from the CPU count and a multiplier, capped by a maximum, and sets the
//! index mask to `size - 1`. The C function allocates the array with
//! `kvmalloc()`; here the caller provides the storage, and [`bucket_locks_size`]
//! gives the size and mask the C function would have chosen.

use core::sync::atomic::{AtomicBool, Ordering};

/// The sizing rule of `__alloc_bucket_spinlocks()`: `(size, mask)`.
///
/// With `cpu_mult == 0` the size is `max_size`. Otherwise it is
/// `min(nr_pcpus, 64) * cpu_mult`, capped at `max_size`. The mask is
/// `size - 1`, so `size` should be a power of two, as in C.
pub fn bucket_locks_size(nr_pcpus: u32, cpu_mult: u32, max_size: u32) -> (u32, u32) {
    let size = if cpu_mult != 0 {
        (nr_pcpus.min(64) * cpu_mult).min(max_size)
    } else {
        max_size
    };
    (size, size.wrapping_sub(1))
}

/// The bucket spinlocks: `storage.len()` locks, selected by `hash & mask`.
#[derive(Debug)]
pub struct BucketLocks<'a> {
    locks: &'a [AtomicBool],
    mask: usize,
}

impl<'a> BucketLocks<'a> {
    /// Use `locks` as the bucket array. Its length must be a power of two, so
    /// that the mask selects every bucket. Panics otherwise.
    pub fn new(locks: &'a [AtomicBool]) -> Self {
        assert!(
            locks.len().is_power_of_two(),
            "bucket count must be a power of two"
        );
        Self {
            locks,
            mask: locks.len() - 1,
        }
    }

    /// The `locks_mask` value C would return.
    pub fn mask(&self) -> usize {
        self.mask
    }

    /// `spin_lock(&locks[hash & mask])`: take the bucket for `hash`. The guard
    /// releases it when dropped.
    pub fn lock(&self, hash: usize) -> BucketGuard<'_> {
        let flag = &self.locks[hash & self.mask];
        while flag
            .compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            core::hint::spin_loop();
        }
        BucketGuard { flag }
    }
}

/// Holds one bucket lock until dropped.
#[derive(Debug)]
pub struct BucketGuard<'a> {
    flag: &'a AtomicBool,
}

impl Drop for BucketGuard<'_> {
    fn drop(&mut self) {
        self.flag.store(false, Ordering::Release);
    }
}

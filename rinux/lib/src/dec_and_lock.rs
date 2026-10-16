// SPDX-License-Identifier: GPL-2.0
//! Drop a reference, taking the lock only when it may be the last one.
//!
//! Ported from `lib/dec_and_lock.c`. `atomic_dec_and_lock()` decrements the
//! counter without the lock unless that would reach zero. Only then does it
//! take the lock and decrement again, so the caller that gets `true` holds the
//! lock and is the one that frees the object.
//!
//! The lock here is an `AtomicBool` spinlock, and the `irqsave` and `raw`
//! variants of the C file are the same algorithm with the kernel's interrupt
//! handling around the lock; those wrap this function in the kernel.

use core::sync::atomic::{AtomicBool, AtomicI32, Ordering};

/// `atomic_dec_and_lock()`: returns `false` with the count dropped and the
/// lock free, or `true` with the count dropped to zero and the lock held.
pub fn atomic_dec_and_lock(count: &AtomicI32, lock: &AtomicBool) -> bool {
    // Subtract 1 unless that would make it 0 (it is 1 now).
    let mut v = count.load(Ordering::Relaxed);
    loop {
        if v == 1 {
            break;
        }
        match count.compare_exchange_weak(v, v - 1, Ordering::Acquire, Ordering::Relaxed) {
            Ok(_) => return false,
            Err(cur) => v = cur,
        }
    }
    // The slow way: take the lock, then decrement for real.
    while lock
        .compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed)
        .is_err()
    {
        core::hint::spin_loop();
    }
    if count.fetch_sub(1, Ordering::AcqRel) - 1 == 0 {
        return true;
    }
    lock.store(false, Ordering::Release);
    false
}

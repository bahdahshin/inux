// SPDX-License-Identifier: GPL-2.0
//! Saturating reference counts.
//!
//! A port of `refcount_t` from `include/linux/refcount.h` and
//! `lib/refcount.c`: an `i32` reference count that detects overflow,
//! underflow and increment-from-zero and pins itself at
//! [`REFCOUNT_SATURATED`] rather than wrapping, so a counting bug leaks memory
//! instead of becoming a use-after-free.
//!
//! Simplified from Linux's: the `WARN` splats become a silent clamp to
//! `REFCOUNT_SATURATED` (rinux has no `refcount_warn_saturate` log path), and
//! the memory-ordering annotations mirror the kernel's but matter only under
//! real concurrency.

use core::sync::atomic::{AtomicI32, Ordering};

/// `REFCOUNT_MAX`: the largest non-saturated value.
pub const REFCOUNT_MAX: i32 = i32::MAX;
/// `REFCOUNT_SATURATED`: the value a broken refcount pins itself at, placed
/// mid-way into the negative range so concurrent operations cannot climb back
/// out of it.
pub const REFCOUNT_SATURATED: i32 = i32::MIN / 2;

/// `refcount_t`: a saturating reference count.
pub struct Refcount {
    refs: AtomicI32,
}

impl Refcount {
    /// `REFCOUNT_INIT()`: a refcount starting at `n`.
    pub const fn new(n: i32) -> Refcount {
        Refcount {
            refs: AtomicI32::new(n),
        }
    }

    /// `refcount_read()`: the current value.
    pub fn read(&self) -> i32 {
        self.refs.load(Ordering::Relaxed)
    }

    /// `refcount_set()`: set the value directly.
    pub fn set(&self, n: i32) {
        self.refs.store(n, Ordering::Relaxed);
    }

    fn saturate(&self) {
        self.refs.store(REFCOUNT_SATURATED, Ordering::Relaxed);
    }

    /// `refcount_add()`: add `i`, saturating on increment-from-zero or
    /// overflow.
    pub fn add(&self, i: i32) {
        let old = self.refs.fetch_add(i, Ordering::Relaxed);
        if old <= 0 || old.wrapping_add(i) < 0 {
            self.saturate();
        }
    }

    /// `refcount_inc()`: add one, saturating on increment-from-zero.
    pub fn inc(&self) {
        self.add(1);
    }

    /// `refcount_add_not_zero()`: add `i` only if the count is non-zero,
    /// returning whether a reference was taken. Saturates on overflow.
    pub fn add_not_zero(&self, i: i32) -> bool {
        let mut old = self.read();
        loop {
            if old == 0 {
                return false;
            }
            match self.refs.compare_exchange_weak(
                old,
                old.wrapping_add(i),
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(cur) => old = cur,
            }
        }
        if old < 0 || old.wrapping_add(i) < 0 {
            self.saturate();
        }
        true
    }

    /// `refcount_inc_not_zero()`: take a reference only if the count is not
    /// already zero.
    pub fn inc_not_zero(&self) -> bool {
        self.add_not_zero(1)
    }

    /// `refcount_sub_and_test()`: subtract `i` and return whether the count
    /// reached zero. Saturates on underflow.
    pub fn sub_and_test(&self, i: i32) -> bool {
        let old = self.refs.fetch_sub(i, Ordering::Release);
        if old > 0 && old == i {
            return true;
        }
        if old <= 0 || old.wrapping_sub(i) < 0 {
            self.saturate();
        }
        false
    }

    /// `refcount_dec_and_test()`: drop a reference and return whether it was
    /// the last.
    pub fn dec_and_test(&self) -> bool {
        self.sub_and_test(1)
    }

    /// `refcount_dec()`: drop a reference the caller knows is not the last;
    /// saturates if it in fact hits zero (a leak) or underflows.
    pub fn dec(&self) {
        let old = self.refs.fetch_sub(1, Ordering::Release);
        if old <= 1 {
            self.saturate();
        }
    }
}

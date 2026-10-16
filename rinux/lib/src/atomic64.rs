// SPDX-License-Identifier: GPL-2.0-or-later
//! 64-bit atomics, as in `lib/atomic64.c`.
//!
//! The C file is the generic fallback for processors without 64-bit atomic
//! instructions: each operation takes a lock from a 16-entry hashed array
//! and returns the value the operation saw. Rust's `AtomicI64` gives the
//! same results with no lock table, so every operation here is a single
//! sequentially consistent atomic instruction. The return values follow the
//! C naming: `fetch_*` returns the old value, `*_return` returns the new one.

use core::sync::atomic::{AtomicI64, Ordering};

/// `atomic64_t`.
#[derive(Debug, Default)]
#[repr(transparent)]
pub struct Atomic64 {
    counter: AtomicI64,
}

impl Atomic64 {
    /// `ATOMIC64_INIT()`.
    pub const fn new(v: i64) -> Atomic64 {
        Atomic64 {
            counter: AtomicI64::new(v),
        }
    }

    /// `generic_atomic64_read()`.
    pub fn read(&self) -> i64 {
        self.counter.load(Ordering::SeqCst)
    }

    /// `generic_atomic64_set()`.
    pub fn set(&self, i: i64) {
        self.counter.store(i, Ordering::SeqCst);
    }

    /// `generic_atomic64_add()`.
    pub fn add(&self, a: i64) {
        self.counter.fetch_add(a, Ordering::SeqCst);
    }

    /// `generic_atomic64_sub()`.
    pub fn sub(&self, a: i64) {
        self.counter.fetch_sub(a, Ordering::SeqCst);
    }

    /// `generic_atomic64_and()`.
    pub fn and(&self, a: i64) {
        self.counter.fetch_and(a, Ordering::SeqCst);
    }

    /// `generic_atomic64_or()`.
    pub fn or(&self, a: i64) {
        self.counter.fetch_or(a, Ordering::SeqCst);
    }

    /// `generic_atomic64_xor()`.
    pub fn xor(&self, a: i64) {
        self.counter.fetch_xor(a, Ordering::SeqCst);
    }

    /// `generic_atomic64_add_return()`: the new value.
    pub fn add_return(&self, a: i64) -> i64 {
        self.counter.fetch_add(a, Ordering::SeqCst).wrapping_add(a)
    }

    /// `generic_atomic64_sub_return()`: the new value.
    pub fn sub_return(&self, a: i64) -> i64 {
        self.counter.fetch_sub(a, Ordering::SeqCst).wrapping_sub(a)
    }

    /// `generic_atomic64_fetch_add()`: the old value.
    pub fn fetch_add(&self, a: i64) -> i64 {
        self.counter.fetch_add(a, Ordering::SeqCst)
    }

    /// `generic_atomic64_fetch_sub()`: the old value.
    pub fn fetch_sub(&self, a: i64) -> i64 {
        self.counter.fetch_sub(a, Ordering::SeqCst)
    }

    /// `generic_atomic64_fetch_and()`: the old value.
    pub fn fetch_and(&self, a: i64) -> i64 {
        self.counter.fetch_and(a, Ordering::SeqCst)
    }

    /// `generic_atomic64_fetch_or()`: the old value.
    pub fn fetch_or(&self, a: i64) -> i64 {
        self.counter.fetch_or(a, Ordering::SeqCst)
    }

    /// `generic_atomic64_fetch_xor()`: the old value.
    pub fn fetch_xor(&self, a: i64) -> i64 {
        self.counter.fetch_xor(a, Ordering::SeqCst)
    }

    /// `atomic64_inc()`.
    pub fn inc(&self) {
        self.add(1);
    }

    /// `atomic64_dec()`.
    pub fn dec(&self) {
        self.sub(1);
    }

    /// `generic_atomic64_dec_if_positive()`: decrements unless the value is
    /// not positive. Returns the value minus one in both cases, as C does,
    /// so a negative result says the counter was left alone.
    pub fn dec_if_positive(&self) -> i64 {
        let mut old = self.counter.load(Ordering::SeqCst);
        loop {
            let val = old.wrapping_sub(1);
            if val < 0 {
                return val;
            }
            match self
                .counter
                .compare_exchange(old, val, Ordering::SeqCst, Ordering::SeqCst)
            {
                Ok(_) => return val,
                Err(now) => old = now,
            }
        }
    }

    /// `generic_atomic64_cmpxchg()`: stores `n` if the value is `o`, and
    /// returns the value seen.
    pub fn cmpxchg(&self, o: i64, n: i64) -> i64 {
        match self
            .counter
            .compare_exchange(o, n, Ordering::SeqCst, Ordering::SeqCst)
        {
            Ok(seen) | Err(seen) => seen,
        }
    }

    /// `generic_atomic64_xchg()`: stores `new` and returns the old value.
    pub fn xchg(&self, new: i64) -> i64 {
        self.counter.swap(new, Ordering::SeqCst)
    }

    /// `generic_atomic64_fetch_add_unless()`: adds `a` unless the value is
    /// `u`, and returns the old value.
    pub fn fetch_add_unless(&self, a: i64, u: i64) -> i64 {
        let mut old = self.counter.load(Ordering::SeqCst);
        loop {
            if old == u {
                return old;
            }
            match self.counter.compare_exchange(
                old,
                old.wrapping_add(a),
                Ordering::SeqCst,
                Ordering::SeqCst,
            ) {
                Ok(_) => return old,
                Err(now) => old = now,
            }
        }
    }

    /// `atomic64_add_unless()`: whether the add happened.
    pub fn add_unless(&self, a: i64, u: i64) -> bool {
        self.fetch_add_unless(a, u) != u
    }

    /// `atomic64_inc_not_zero()`.
    pub fn inc_not_zero(&self) -> bool {
        self.add_unless(1, 0)
    }
}

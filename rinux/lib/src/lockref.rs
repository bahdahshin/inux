// SPDX-License-Identifier: GPL-2.0
//! A reference count that is updated without taking its lock when it can.
//!
//! Ported from `lib/lockref.c` and `include/linux/lockref.h`. C keeps the lock
//! and the count in one 64-bit word, so one `cmpxchg` can change the count
//! when the lock is free. Here that word is an `AtomicU64`: the low 32 bits
//! are the lock (0 free, 1 held) and the high 32 bits are the signed count.
//! The fast path is the `CMPXCHG_LOOP` of `lockref.c`, which retries up to 100
//! times and then falls back to taking the lock, as C does.
//!
//! Two behaviours are kept from C on purpose. `put_return()` gives up with -1
//! without taking the lock when the fast path fails. `put_or_lock()` returns
//! with the lock still held when it reports "not the last reference", so the
//! caller must call [`Lockref::unlock`].

use core::sync::atomic::{AtomicU64, Ordering};

/// `__LOCKREF_DEAD_VAL`: the count once the object is marked dead.
pub const LOCKREF_DEAD_VAL: i32 = -128;

const LOCK_BIT: u64 = 1;
const RETRIES: u32 = 100;

/// `struct lockref`: a lock and a signed count in one word.
#[derive(Debug, Default)]
pub struct Lockref {
    word: AtomicU64,
}

/// What [`Lockref::put_or_lock`] did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PutOrLock {
    /// The count was decremented and the lock is free.
    Put,
    /// This was the last reference: the lock is held and the count is
    /// unchanged. The caller must [`Lockref::unlock`].
    LockHeld,
}

fn count_of(word: u64) -> i32 {
    (word >> 32) as i32
}

fn with_count(word: u64, count: i32) -> u64 {
    (word & 0xffff_ffff) | (u64::from(count as u32) << 32)
}

fn add(delta: i32) -> u64 {
    (delta as i64 as u64) << 32
}

impl Lockref {
    /// A lockref with count `count` and the lock free.
    pub const fn new(count: i32) -> Self {
        Self {
            word: AtomicU64::new((count as u32 as u64) << 32),
        }
    }

    /// The count, read without the lock (as `READ_ONCE()` does in C).
    pub fn count(&self) -> i32 {
        count_of(self.word.load(Ordering::Relaxed))
    }

    /// `spin_lock()` on the embedded lock: spins until the lock bit is taken.
    pub fn lock(&self) {
        loop {
            let cur = self.word.load(Ordering::Relaxed);
            if cur & LOCK_BIT == 0
                && self
                    .word
                    .compare_exchange_weak(
                        cur,
                        cur | LOCK_BIT,
                        Ordering::Acquire,
                        Ordering::Relaxed,
                    )
                    .is_ok()
            {
                return;
            }
            core::hint::spin_loop();
        }
    }

    /// `spin_unlock()`: frees the lock bit and keeps the count.
    pub fn unlock(&self) {
        self.word.fetch_and(!LOCK_BIT, Ordering::Release);
    }

    /// The `CMPXCHG_LOOP` of `lockref.c`: run `code` on the current count,
    /// which returns `None` to fall back to the lock, or `Some(result)` to
    /// stop. The change is committed with `cmpxchg` while the lock is free.
    fn cmpxchg_loop<R>(&self, mut code: impl FnMut(i32) -> Option<(u64, R)>) -> Option<R> {
        let mut old = self.word.load(Ordering::Relaxed);
        for _ in 0..RETRIES {
            if old & LOCK_BIT != 0 {
                return None;
            }
            let (delta_word, result) = code(count_of(old))?;
            let new = old.wrapping_add(delta_word);
            match self
                .word
                .compare_exchange_weak(old, new, Ordering::Relaxed, Ordering::Relaxed)
            {
                Ok(_) => return Some(result),
                Err(cur) => old = cur,
            }
        }
        None
    }

    /// `lockref_get()`: count one more reference.
    pub fn get(&self) {
        if self.cmpxchg_loop(|_| Some((add(1), ()))).is_some() {
            return;
        }
        self.lock();
        self.word.fetch_add(add(1), Ordering::Relaxed);
        self.unlock();
    }

    /// `lockref_get_not_zero()`: count one more reference if the count is
    /// positive.
    pub fn get_not_zero(&self) -> bool {
        let fast = self.cmpxchg_loop(|count| {
            if count <= 0 {
                // C returns false from inside the loop, so do not fall back.
                return Some((0, false));
            }
            Some((add(1), true))
        });
        if let Some(found) = fast {
            return found;
        }
        self.lock();
        let retval = if count_of(self.word.load(Ordering::Relaxed)) > 0 {
            self.word.fetch_add(add(1), Ordering::Relaxed);
            true
        } else {
            false
        };
        self.unlock();
        retval
    }

    /// `lockref_put_return()`: drop one reference and return the new count.
    /// Returns -1 without taking the lock when the fast path does not apply,
    /// as C does.
    pub fn put_return(&self) -> i32 {
        self.cmpxchg_loop(|count| {
            if count <= 0 {
                return Some((0, -1));
            }
            Some((add(-1), count - 1))
        })
        .unwrap_or(-1)
    }

    /// `lockref_put_or_lock()`: drop a reference unless it is the last one.
    pub fn put_or_lock(&self) -> PutOrLock {
        let fast = self.cmpxchg_loop(|count| {
            if count <= 1 {
                // Last reference (or worse): take the slow path.
                return None;
            }
            Some((add(-1), PutOrLock::Put))
        });
        if let Some(done) = fast {
            return done;
        }
        self.lock();
        if count_of(self.word.load(Ordering::Relaxed)) <= 1 {
            // Matches C: the lock stays held for the caller.
            return PutOrLock::LockHeld;
        }
        self.word.fetch_add(add(-1), Ordering::Relaxed);
        self.unlock();
        PutOrLock::Put
    }

    /// `lockref_mark_dead()`: the caller must hold the lock.
    pub fn mark_dead(&self) {
        let old = self.word.load(Ordering::Relaxed);
        self.word
            .store(with_count(old, LOCKREF_DEAD_VAL), Ordering::Relaxed);
    }

    /// `lockref_get_not_dead()`: count one more reference unless the object
    /// is dead (a negative count).
    pub fn get_not_dead(&self) -> bool {
        let fast = self.cmpxchg_loop(|count| {
            if count < 0 {
                return Some((0, false));
            }
            Some((add(1), true))
        });
        if let Some(found) = fast {
            return found;
        }
        self.lock();
        let retval = if count_of(self.word.load(Ordering::Relaxed)) >= 0 {
            self.word.fetch_add(add(1), Ordering::Relaxed);
            true
        } else {
            false
        };
        self.unlock();
        retval
    }
}

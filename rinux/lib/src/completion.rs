// SPDX-License-Identifier: GPL-2.0-only
//! Completions: a counter that a producer bumps and a consumer takes, with
//! `complete_all()` making the completion permanent.
//!
//! Ported from `kernel/sched/completion.c`: `complete()`, `complete_all()`,
//! `try_wait_for_completion()` and `completion_done()`, over the `done` count.
//! `UINT_MAX` in C means "completed for everyone", and it is not decremented.
//! Blocking waits belong to the scheduler and are not ported. The counter is an
//! atomic so that a completer and a waiter on different CPUs agree on it.

use core::sync::atomic::{AtomicU32, Ordering};

/// `UINT_MAX`: the done count after `complete_all()`.
const DONE_ALL: u32 = u32::MAX;

/// `struct completion`, with the count only; the wait queue belongs to the
/// scheduler.
#[derive(Debug, Default)]
pub struct Completion {
    done: AtomicU32,
}

impl Completion {
    /// `init_completion()`: nothing done yet.
    pub const fn new() -> Self {
        Self {
            done: AtomicU32::new(0),
        }
    }

    /// `complete()`: let one waiter through. The count saturates below
    /// `UINT_MAX`, as C does.
    pub fn complete(&self) {
        let mut cur = self.done.load(Ordering::Relaxed);
        while cur != DONE_ALL {
            match self.done.compare_exchange_weak(
                cur,
                cur + 1,
                Ordering::Release,
                Ordering::Relaxed,
            ) {
                Ok(_) => return,
                Err(actual) => cur = actual,
            }
        }
    }

    /// `complete_all()`: every current and future waiter passes.
    pub fn complete_all(&self) {
        self.done.store(DONE_ALL, Ordering::Release);
    }

    /// `try_wait_for_completion()`: take one completion if there is one, without
    /// blocking. A `complete_all()` is never used up.
    pub fn try_wait(&self) -> bool {
        let mut cur = self.done.load(Ordering::Acquire);
        loop {
            if cur == 0 {
                return false;
            }
            if cur == DONE_ALL {
                return true;
            }
            match self
                .done
                .compare_exchange_weak(cur, cur - 1, Ordering::AcqRel, Ordering::Acquire)
            {
                Ok(_) => return true,
                Err(actual) => cur = actual,
            }
        }
    }

    /// `completion_done()`: whether a completion is pending.
    pub fn is_done(&self) -> bool {
        self.done.load(Ordering::Acquire) != 0
    }
}

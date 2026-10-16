// SPDX-License-Identifier: GPL-2.0
//! Error sequence numbers: record an error once and let any number of
//! subscribers tell whether it has changed since they last looked.
//!
//! Ported from `lib/errseq.c` and `include/linux/errseq.h`. An `errseq_t` is
//! a `u32`: the low bits hold the error (`-1` to `-MAX_ERRNO`), a `SEEN`
//! flag says a subscriber has sampled it, and the bits above count updates.
//! The C code uses `cmpxchg()`; here it is `AtomicU32::compare_exchange`, so
//! the functions work from any context. Callers keep the value in an
//! [`AtomicU32`] that starts at zero, meaning no error has ever been set.

use crate::errno::MAX_ERRNO;
use core::sync::atomic::{AtomicU32, Ordering};

/// `errseq_t`: an error code, a `SEEN` flag and a sequence counter.
pub type Errseq = u32;

/// `ERRSEQ_SHIFT`: the first bit above the error code.
const ERRSEQ_SHIFT: u32 = (MAX_ERRNO as u32).ilog2() + 1;
/// `ERRSEQ_SEEN`: set once a subscriber has sampled the current error.
const ERRSEQ_SEEN: u32 = 1 << ERRSEQ_SHIFT;
/// `ERRNO_MASK`: the error code bits.
const ERRNO_MASK: u32 = ERRSEQ_SEEN - 1;
/// `ERRSEQ_CTR_INC`: one step of the sequence counter.
const ERRSEQ_CTR_INC: u32 = 1 << (ERRSEQ_SHIFT + 1);

/// `errseq_set()`: record `err` (`-1` to `-MAX_ERRNO`) for later reporting.
///
/// The counter moves on only if some subscriber has sampled the value since
/// the last error was recorded. Returns the previous value; as in C it is
/// for debugging, since it lacks a usable `SEEN` flag. An invalid `err`
/// (zero, positive, or below `-MAX_ERRNO`) leaves the value alone.
pub fn errseq_set(eseq: &AtomicU32, err: i32) -> Errseq {
    let mut old = eseq.load(Ordering::Relaxed);

    // `(unsigned int)-err > MAX_ERRNO` in C: zero and positive values wrap
    // to a large unsigned number here as well.
    if err == 0 || err.wrapping_neg() as u32 > MAX_ERRNO as u32 {
        // C warns with WARN(); the caller can see it from the return value.
        return old;
    }

    loop {
        // Clear the old error and set the new one.
        let mut new = (old & !(ERRNO_MASK | ERRSEQ_SEEN)) | err.wrapping_neg() as u32;

        // Only bump the counter if someone has looked at the old value.
        if old & ERRSEQ_SEEN != 0 {
            new = new.wrapping_add(ERRSEQ_CTR_INC);
        }

        // Nothing would change, so we are done.
        if new == old {
            return new;
        }

        match eseq.compare_exchange(old, new, Ordering::SeqCst, Ordering::Relaxed) {
            Ok(_) => return old,
            Err(cur) => {
                // Someone else stored the same value, or we raced: the C
                // code accepts either and otherwise retries.
                if cur == new {
                    return cur;
                }
                old = cur;
            }
        }
    }
}

/// `errseq_sample()`: the value a new subscriber should start from.
///
/// If the current error has been seen, new subscribers start from zero so
/// they do not report an old error. An unseen error is returned unchanged
/// so the subscriber reports it on its next check.
pub fn errseq_sample(eseq: &AtomicU32) -> Errseq {
    let old = eseq.load(Ordering::Relaxed);
    if old & ERRSEQ_SEEN == 0 {
        // Nobody has seen this error yet, so we can be the first.
        return 0;
    }
    old
}

/// `errseq_check()`: the error recorded since `since`, or `0` if nothing has
/// changed. `since` is not advanced.
pub fn errseq_check(eseq: &AtomicU32, since: Errseq) -> i32 {
    let cur = eseq.load(Ordering::Relaxed);
    if cur == since {
        return 0;
    }
    -((cur & ERRNO_MASK) as i32)
}

/// `errseq_check_and_advance()`: like [`errseq_check`], but also marks the
/// current error as seen and moves `since` to the new value.
///
/// Racing writers can make the swap fail; as in C that is harmless, since
/// `since` still advances to what this call observed.
pub fn errseq_check_and_advance(eseq: &AtomicU32, since: &mut Errseq) -> i32 {
    let mut err = 0;
    let old = eseq.load(Ordering::Relaxed);
    if old != *since {
        let new = old | ERRSEQ_SEEN;
        if new != old {
            // The result does not matter; see the doc comment above.
            let _ = eseq.compare_exchange(old, new, Ordering::SeqCst, Ordering::Relaxed);
        }
        *since = new;
        err = -((new & ERRNO_MASK) as i32);
    }
    err
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_matches_the_c_macros() {
        // ilog2(4095) + 1 = 12, so the error uses the low 12 bits.
        assert_eq!(ERRSEQ_SHIFT, 12);
        assert_eq!(ERRSEQ_SEEN, 0x1000);
        assert_eq!(ERRNO_MASK, 0x0fff);
        assert_eq!(ERRSEQ_CTR_INC, 0x2000);
    }
}

// SPDX-License-Identifier: GPL-2.0
//! Mark the kernel as busy printing an oops, so output is not blocked by
//! locks, and undo that on exit.
//!
//! Ported from `lib/bust_spinlocks.c`. The C function bumps a global
//! `oops_in_progress` counter and, on exit, unblanks the console and wakes
//! the log daemon once the counter reaches zero. The console and klogd
//! belong to the kernel parts rinux has not ported yet, so the caller passes
//! them in as closures.

use core::sync::atomic::{AtomicI32, Ordering};

/// `oops_in_progress`: how many nested oopses are printing.
pub static OOPS_IN_PROGRESS: AtomicI32 = AtomicI32::new(0);

/// `bust_spinlocks()`: `yes` enters the mode, `!yes` leaves it.
///
/// Leaving always calls `console_unblank`. It calls `wake_klogd` only when
/// the last nested oops has finished, as in C.
pub fn bust_spinlocks(yes: bool, console_unblank: impl FnOnce(), wake_klogd: impl FnOnce()) {
    if yes {
        OOPS_IN_PROGRESS.fetch_add(1, Ordering::SeqCst);
    } else {
        console_unblank();
        if OOPS_IN_PROGRESS.fetch_sub(1, Ordering::SeqCst) - 1 == 0 {
            wake_klogd();
        }
    }
}

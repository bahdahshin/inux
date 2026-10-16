// SPDX-License-Identifier: GPL-2.0-only
//! The scheduler clock.
//!
//! A partial port of `kernel/sched/clock.c`. With one CPU and a stable TSC,
//! `local_clock()` is `sched_clock()`; the per-CPU drift correction for
//! unstable clocks comes with SMP.

/// `sched_clock()`: nanoseconds since boot, from the architecture.
#[inline]
pub fn sched_clock() -> u64 {
    crate::arch::x86::kernel::tsc::native_sched_clock()
}

/// `local_clock()`: this CPU's clock, used for printk timestamps.
#[inline]
pub fn local_clock() -> u64 {
    sched_clock()
}

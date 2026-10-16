// SPDX-License-Identifier: GPL-2.0
//! The periodic tick.
//!
//! A partial port of `kernel/time/tick-common.c` and `do_timer()` from
//! `kernel/time/timekeeping.c`. Wall time, timer wheels and scheduler
//! ticks come later.

use core::sync::atomic::Ordering;

use super::jiffies::JIFFIES_64;

/// `do_timer()`: advances jiffies.
pub fn do_timer(ticks: u64) {
    JIFFIES_64.fetch_add(ticks, Ordering::Relaxed);
}

/// `tick_periodic()`: the work of one tick on the timekeeping CPU.
fn tick_periodic() {
    do_timer(1);
    // update_process_times() -> sched_tick()
    crate::kernel::sched::core::scheduler_tick();
}

/// `tick_handle_periodic()`: the clock event handler in periodic mode.
pub fn tick_handle_periodic() {
    tick_periodic();
}

// SPDX-License-Identifier: GPL-2.0-only
//! Interrupt context tracking.
//!
//! A partial port of `irq_enter()`/`irq_exit()` from `kernel/softirq.c`
//! and the `in_interrupt()` test of `include/linux/preempt.h`. Linux keeps
//! this in `preempt_count`; with one CPU and no preemption a counter will
//! do. Softirqs come later.

use core::sync::atomic::{AtomicU32, Ordering};

static HARDIRQ_COUNT: AtomicU32 = AtomicU32::new(0);

/// `irq_enter()`: entering a hardware interrupt handler.
pub fn irq_enter() {
    HARDIRQ_COUNT.fetch_add(1, Ordering::Relaxed);
}

/// `irq_exit()`: leaving a hardware interrupt handler.
pub fn irq_exit() {
    HARDIRQ_COUNT.fetch_sub(1, Ordering::Relaxed);
}

/// `in_interrupt()`: running in interrupt context.
pub fn in_interrupt() -> bool {
    HARDIRQ_COUNT.load(Ordering::Relaxed) != 0
}

/// `preempt_count`: nonzero in any non-preemptible section. Bumped by
/// spinlocks (`spin_lock()` disables preemption) and by interrupt entry.
static PREEMPT_COUNT: AtomicU32 = AtomicU32::new(0);

/// `preempt_disable()`.
#[inline]
pub fn preempt_disable() {
    PREEMPT_COUNT.fetch_add(1, Ordering::Relaxed);
}

/// `preempt_enable()`.
#[inline]
pub fn preempt_enable() {
    PREEMPT_COUNT.fetch_sub(1, Ordering::Relaxed);
}

/// `preempt_count()`.
#[inline]
pub fn preempt_count() -> u32 {
    PREEMPT_COUNT.load(Ordering::Relaxed)
}

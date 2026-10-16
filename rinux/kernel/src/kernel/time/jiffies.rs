// SPDX-License-Identifier: GPL-2.0
//! `jiffies`: ticks since boot.
//!
//! Ported from `include/linux/jiffies.h`, `include/vdso/jiffies.h` and
//! `kernel/time/jiffies.c`.

use core::sync::atomic::{AtomicU64, Ordering};

/// `HZ`: timer interrupts per second (`CONFIG_HZ_1000` in x86_64_defconfig).
pub const HZ: u64 = 1000;

/// `INITIAL_JIFFIES`: jiffies starts five minutes before wrapping its low
/// 32 bits, so wraparound bugs show up early.
pub const INITIAL_JIFFIES: u64 = (-300i32 * HZ as i32) as u32 as u64;

/// `jiffies_64`.
pub static JIFFIES_64: AtomicU64 = AtomicU64::new(INITIAL_JIFFIES);

/// `jiffies` / `get_jiffies_64()`.
#[inline]
pub fn jiffies() -> u64 {
    JIFFIES_64.load(Ordering::Relaxed)
}

/// `jiffies_to_msecs()` for `HZ` = 1000.
#[inline]
pub const fn jiffies_to_msecs(j: u64) -> u64 {
    j * 1000 / HZ
}

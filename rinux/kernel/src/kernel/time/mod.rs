// SPDX-License-Identifier: GPL-2.0
//! Timekeeping, as in Linux's `kernel/time/`.

pub mod clocksource;
pub mod jiffies;
pub mod sleep_timeout;
pub mod tick_common;
pub mod timekeeping;

/// `NSEC_PER_SEC`.
pub const NSEC_PER_SEC: u64 = 1_000_000_000;
/// `NSEC_PER_MSEC`.
pub const NSEC_PER_MSEC: u64 = 1_000_000;
/// `NSEC_PER_USEC`.
pub const NSEC_PER_USEC: u64 = 1000;
/// `USEC_PER_MSEC`.
pub const USEC_PER_MSEC: u64 = 1000;
/// `KHZ`.
pub const KHZ: u64 = 1000;

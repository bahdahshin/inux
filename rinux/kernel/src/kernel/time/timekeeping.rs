// SPDX-License-Identifier: GPL-2.0
//! Wall-clock timekeeping.
//!
//! A small port of `kernel/time/timekeeping.c`: at boot, read the persistent
//! clock (the RTC) once to anchor CLOCK_REALTIME, remembering the jiffies count
//! at that instant. The current real time is then that anchor plus the elapsed
//! jiffies, exactly as `timekeeping_init()` seeds `tk_core` from
//! `read_persistent_clock64()` and `ktime_get_real_ts64()` adds the monotonic
//! delta since.
//!
//! Simplified heavily: a single tick granularity (jiffies, no clocksource
//! sub-tick interpolation), no NTP adjustment, no suspend/resume handling, and
//! no leap-second logic.

use core::sync::atomic::{AtomicI64, AtomicU64, Ordering};

use rinux_lib::mktime::mktime64;

use super::NSEC_PER_SEC;
use super::jiffies::{HZ, jiffies};
use crate::drivers::rtc::rtc_cmos::read_time;

/// The CLOCK_REALTIME seconds value read from the RTC at boot.
static BOOT_WALL_SEC: AtomicI64 = AtomicI64::new(0);
/// The jiffies count sampled at the same instant as `BOOT_WALL_SEC`.
static BOOT_JIFFIES: AtomicU64 = AtomicU64::new(0);

/// A time in seconds and nanoseconds since the Unix epoch, as `struct
/// timespec64`.
pub struct Timespec64 {
    pub tv_sec: i64,
    pub tv_nsec: i64,
}

/// `timekeeping_init()`: anchor CLOCK_REALTIME to the persistent clock.
///
/// Reads the RTC and converts it to a Unix timestamp with `mktime64()`,
/// sampling jiffies at the same time so the elapsed monotonic time can be
/// added back later.
pub fn timekeeping_init() {
    let t = read_time();
    let wall = mktime64(
        t.year,
        t.month as u32,
        t.mday as u32,
        t.hour as u32,
        t.min as u32,
        t.sec as u32,
    );
    // Sample jiffies right after the RTC read; the tiny read latency is well
    // under one tick.
    BOOT_JIFFIES.store(jiffies(), Ordering::Relaxed);
    BOOT_WALL_SEC.store(wall, Ordering::Relaxed);
}

/// `ktime_get_real_ts64()`: the current CLOCK_REALTIME time.
pub fn ktime_get_real_ts64() -> Timespec64 {
    let elapsed = jiffies().wrapping_sub(BOOT_JIFFIES.load(Ordering::Relaxed));
    let sec = BOOT_WALL_SEC.load(Ordering::Relaxed) + (elapsed / HZ) as i64;
    let nsec = (elapsed % HZ) as i64 * (NSEC_PER_SEC / HZ) as i64;
    Timespec64 {
        tv_sec: sec,
        tv_nsec: nsec,
    }
}

/// `ktime_get_real_seconds()`: the current CLOCK_REALTIME whole seconds.
pub fn ktime_get_real_seconds() -> i64 {
    ktime_get_real_ts64().tv_sec
}

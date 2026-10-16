// SPDX-License-Identifier: GPL-2.0
//! Event rate limiting.
//!
//! A port of `___ratelimit()` from `lib/ratelimit.c` with `struct
//! ratelimit_state` from `include/linux/ratelimit_types.h`: allow up to
//! `burst` events in each `interval`-long window and suppress the rest,
//! counting how many were suppressed (what `printk_ratelimited` is built on).
//!
//! Simplified from Linux's: the current time is passed in rather than read
//! from `jiffies` (so the logic is pure and host-testable), and the SMP lock
//! and atomics are dropped -- rinux calls this from a single context. The
//! "N callbacks suppressed" message is replaced by the queryable
//! [`missed`](RatelimitState::missed) count.

/// `struct ratelimit_state`: the running state of one rate limiter.
pub struct RatelimitState {
    /// The window length, in the same units as the time passed to
    /// [`ratelimit`](Self::ratelimit) (jiffies in the kernel).
    pub interval: i32,
    /// How many events are allowed per window.
    pub burst: i32,
    /// Events still allowed in the current window.
    n_left: i32,
    /// Events suppressed since the last window reset.
    missed: u32,
    /// Whether the window has been started.
    initialized: bool,
    /// The time the current window began.
    begin: u64,
}

impl RatelimitState {
    /// `DEFINE_RATELIMIT_STATE()`: a limiter allowing `burst` events every
    /// `interval` time units (Linux's defaults are `5 * HZ` and `10`).
    pub const fn new(interval: i32, burst: i32) -> RatelimitState {
        RatelimitState {
            interval,
            burst,
            n_left: 0,
            missed: 0,
            initialized: false,
            begin: 0,
        }
    }

    /// Events suppressed in the current window so far.
    pub fn missed(&self) -> u32 {
        self.missed
    }

    /// `___ratelimit()`: record an event at time `now`, returning `true` if it
    /// is allowed and `false` if it is suppressed.
    pub fn ratelimit(&mut self, now: u64) -> bool {
        let interval = self.interval;
        let burst = self.burst;

        // A zero interval never limits; a non-positive burst always limits.
        if interval <= 0 || burst <= 0 {
            let ret = interval == 0 || burst > 0;
            // The kernel forces re-initialization once re-enabled.
            self.initialized = false;
            if !ret {
                self.missed = self.missed.wrapping_add(1);
            }
            return ret;
        }

        if !self.initialized {
            self.begin = now;
            self.initialized = true;
            self.n_left = burst;
        }

        // time_is_before_jiffies(begin + interval): the window has elapsed.
        if now > self.begin.wrapping_add(interval as u64) {
            self.n_left = burst;
            self.begin = now;
            // The suppressed count is "reported" and cleared each window.
            self.missed = 0;
        }

        if self.n_left > 0 {
            self.n_left -= 1;
            true
        } else {
            self.missed = self.missed.wrapping_add(1);
            false
        }
    }
}

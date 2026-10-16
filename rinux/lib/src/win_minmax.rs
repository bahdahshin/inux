// SPDX-License-Identifier: GPL-2.0
//! Windowed min/max tracker.
//!
//! A port of `lib/win_minmax.c` and `include/linux/win_minmax.h`: Kathleen
//! Nichols' algorithm for tracking the minimum (or maximum) of a stream over
//! a fixed time window in constant space and time (e.g. the minimum RTT over
//! the last few minutes, as TCP congestion control uses). It keeps the best,
//! 2nd- and 3rd-best samples, spread across the window, which almost always
//! gives the same answer as keeping every sample.
//!
//! Times are `u32` and compared with wrapping arithmetic, so the window works
//! across counter wraparound.

/// A measured value and the time it was taken.
#[derive(Clone, Copy, Default)]
pub struct MinmaxSample {
    pub t: u32,
    pub v: u32,
}

/// The running min/max estimator state.
#[derive(Clone, Copy, Default)]
pub struct Minmax {
    s: [MinmaxSample; 3],
}

impl Minmax {
    /// An estimator with all samples zeroed.
    pub const fn new() -> Minmax {
        Minmax {
            s: [MinmaxSample { t: 0, v: 0 }; 3],
        }
    }

    /// `minmax_get()`: the current best (min or max) value.
    pub fn get(&self) -> u32 {
        self.s[0].v
    }

    /// `minmax_reset()`: forget all history and start from `meas` at `t`.
    pub fn reset(&mut self, t: u32, meas: u32) -> u32 {
        let val = MinmaxSample { t, v: meas };
        self.s = [val, val, val];
        self.s[0].v
    }

    /// `minmax_subwin_update()`: as time advances, roll the 1st/2nd/3rd
    /// choices forward so they stay spread across the window.
    fn subwin_update(&mut self, win: u32, val: &MinmaxSample) -> u32 {
        let dt = val.t.wrapping_sub(self.s[0].t);
        if dt > win {
            // The best is older than the window; promote the others.
            self.s[0] = self.s[1];
            self.s[1] = self.s[2];
            self.s[2] = *val;
            if val.t.wrapping_sub(self.s[0].t) > win {
                self.s[0] = self.s[1];
                self.s[1] = self.s[2];
                self.s[2] = *val;
            }
        } else if self.s[1].t == self.s[0].t && dt > win / 4 {
            // A quarter window with no new best: take a 2nd choice.
            self.s[2] = *val;
            self.s[1] = *val;
        } else if self.s[2].t == self.s[1].t && dt > win / 2 {
            // Half a window: take a 3rd choice.
            self.s[2] = *val;
        }
        self.s[0].v
    }

    /// `minmax_running_max()`: feed `meas` at time `t`, returning the maximum
    /// over the last `win` time units.
    pub fn running_max(&mut self, win: u32, t: u32, meas: u32) -> u32 {
        let val = MinmaxSample { t, v: meas };
        if val.v >= self.s[0].v || val.t.wrapping_sub(self.s[2].t) > win {
            return self.reset(t, meas);
        }
        if val.v >= self.s[1].v {
            self.s[2] = val;
            self.s[1] = val;
        } else if val.v >= self.s[2].v {
            self.s[2] = val;
        }
        self.subwin_update(win, &val)
    }

    /// `minmax_running_min()`: feed `meas` at time `t`, returning the minimum
    /// over the last `win` time units.
    pub fn running_min(&mut self, win: u32, t: u32, meas: u32) -> u32 {
        let val = MinmaxSample { t, v: meas };
        if val.v <= self.s[0].v || val.t.wrapping_sub(self.s[2].t) > win {
            return self.reset(t, meas);
        }
        if val.v <= self.s[1].v {
            self.s[2] = val;
            self.s[1] = val;
        } else if val.v <= self.s[2].v {
            self.s[2] = val;
        }
        self.subwin_update(win, &val)
    }
}

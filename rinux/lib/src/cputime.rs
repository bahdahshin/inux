// SPDX-License-Identifier: GPL-2.0-only
//! Split a task's total runtime into user and system time, keeping both
//! monotonic and in the sampled ratio.
//!
//! Ported from `kernel/sched/cputime.c`, `cputime_adjust()`. The kernel samples
//! user and system time from ticks, which is coarse; the total runtime is
//! precise. The adjustment scales the precise total by the tick ratio, then
//! clamps each part so that neither ever goes backwards for a reader. The
//! per-task lock in C serialises callers and is the caller's here, since the
//! `prev` state is passed in explicitly.

/// `struct task_cputime`, the sample the adjustment reads.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CputimeSample {
    /// User time, from the ticks.
    pub utime: u64,
    /// System time, from the ticks.
    pub stime: u64,
    /// Total runtime, precise.
    pub sum_exec_runtime: u64,
}

/// `struct prev_cputime`: the values last reported, which bound the next ones.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PrevCputime {
    /// Last reported user time.
    pub utime: u64,
    /// Last reported system time.
    pub stime: u64,
}

/// `cputime_adjust()`: the `(utime, stime)` to report, and the updated `prev`.
pub fn cputime_adjust(curr: CputimeSample, prev: &mut PrevCputime) -> (u64, u64) {
    let rtime = curr.sum_exec_runtime;

    // Either the total went backwards or a caller raced us: keep what we had.
    if prev.stime + prev.utime >= rtime {
        return (prev.utime, prev.stime);
    }

    let mut stime;

    if curr.stime == 0 {
        // No system ticks yet: assume all runtime is user time.
        stime = 0;
    } else if curr.utime == 0 {
        stime = rtime;
    } else {
        stime = ((u128::from(curr.stime) * u128::from(rtime)) / u128::from(curr.stime + curr.utime))
            as u64;
    }
    // The part of the code that makes stime never go backwards keeps utime
    // monotonic too, since rtime is.
    if stime < prev.stime {
        stime = prev.stime;
    }
    let mut utime = rtime - stime;

    if utime < prev.utime {
        utime = prev.utime;
        stime = rtime - utime;
    }

    prev.stime = stime;
    prev.utime = utime;
    (prev.utime, prev.stime)
}

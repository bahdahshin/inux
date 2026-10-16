// SPDX-License-Identifier: GPL-2.0-only
//! Scheduler priority numbers: the nice and real-time ranges, and the normal
//! priority a task gets from its policy.
//!
//! Ported from `include/linux/sched/prio.h` (the constants and the nice and
//! priority conversions) and `__normal_prio()` in `kernel/sched/syscalls.c`.
//! Lower numbers are higher priority: deadline tasks are at -1, real-time tasks
//! at 0..=99 with the highest real-time priority at 0, and ordinary tasks at
//! 100 + (nice + 20).

/// `MAX_NICE`.
pub const MAX_NICE: i32 = 19;
/// `MIN_NICE`.
pub const MIN_NICE: i32 = -20;
/// `NICE_WIDTH`: the number of nice levels.
pub const NICE_WIDTH: i32 = MAX_NICE - MIN_NICE + 1;
/// `MAX_RT_PRIO`: the number of real-time priorities.
pub const MAX_RT_PRIO: i32 = 100;
/// `MAX_DL_PRIO`: deadline tasks sit just below the real-time range.
pub const MAX_DL_PRIO: i32 = 0;
/// `MAX_PRIO`: one past the lowest priority.
pub const MAX_PRIO: i32 = MAX_RT_PRIO + NICE_WIDTH;
/// `DEFAULT_PRIO`: the priority of a nice-0 task.
pub const DEFAULT_PRIO: i32 = MAX_RT_PRIO + NICE_WIDTH / 2;

/// `NICE_TO_PRIO()`.
pub const fn nice_to_prio(nice: i32) -> i32 {
    nice + DEFAULT_PRIO
}

/// `PRIO_TO_NICE()`.
pub const fn prio_to_nice(prio: i32) -> i32 {
    prio - DEFAULT_PRIO
}

/// `SCHED_FIFO`, `SCHED_RR` and `SCHED_DEADLINE`: the policy numbers of the
/// real-time and deadline classes.
pub const SCHED_FIFO: i32 = 1;
/// `SCHED_RR`.
pub const SCHED_RR: i32 = 2;
/// `SCHED_DEADLINE`.
pub const SCHED_DEADLINE: i32 = 6;

/// `__normal_prio()`: the priority a task has from its policy, before any
/// inheritance: deadline tasks first, then real-time by `rt_prio`, otherwise the
/// nice level.
pub fn normal_prio(policy: i32, rt_prio: i32, nice: i32) -> i32 {
    if policy == SCHED_DEADLINE {
        MAX_DL_PRIO - 1
    } else if policy == SCHED_FIFO || policy == SCHED_RR {
        MAX_RT_PRIO - 1 - rt_prio
    } else {
        nice_to_prio(nice)
    }
}

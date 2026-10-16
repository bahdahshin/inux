// SPDX-License-Identifier: GPL-2.0-only
//! Map a task priority onto the CPU-priority scale that cpupri tracks.
//!
//! Ported from `kernel/sched/cpupri.c`, `convert_prio()`, and the constants of
//! `kernel/sched/cpupri.h`. The scale runs from INVALID (-1, not routable)
//! through NORMAL (0) and the real-time levels 1 to 99, to HIGHER (100). A
//! task's kernel priority (0 is the highest real-time) is turned around, so a
//! higher number here means a more urgent CPU state. The two-dimensional bitmap
//! that cpupri keeps over these levels, and the search over it, are not ported
//! here.

/// `CPUPRI_NR_PRIORITIES`: the number of levels.
pub const CPUPRI_NR_PRIORITIES: i32 = 101;
/// `CPUPRI_INVALID`: the CPU is not eligible for routing.
pub const CPUPRI_INVALID: i32 = -1;
/// `CPUPRI_NORMAL`: a CPU running only ordinary tasks.
pub const CPUPRI_NORMAL: i32 = 0;
/// `CPUPRI_HIGHER`: a CPU running a task more urgent than real-time.
pub const CPUPRI_HIGHER: i32 = 100;
/// `MAX_RT_PRIO`.
const MAX_RT_PRIO: i32 = 100;

/// `convert_prio()`: the cpupri level for a task priority, or `None` for a
/// priority C's switch does not cover (below -1 or above 100).
pub fn convert_prio(prio: i32) -> Option<i32> {
    match prio {
        CPUPRI_INVALID => Some(CPUPRI_INVALID),
        0..=98 => Some(MAX_RT_PRIO - 1 - prio),
        x if x == MAX_RT_PRIO - 1 => Some(CPUPRI_NORMAL),
        x if x == MAX_RT_PRIO => Some(CPUPRI_HIGHER),
        _ => None,
    }
}

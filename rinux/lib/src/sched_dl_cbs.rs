// SPDX-License-Identifier: GPL-2.0-only
//! The constant-bandwidth server rules of SCHED_DEADLINE: replenishing a
//! task's budget and moving its deadline, and the overflow test that says the
//! task has used more than its share.
//!
//! Ported from `kernel/sched/deadline.c`: `replenish_dl_new_period()`,
//! `replenish_dl_entity()` and `dl_entity_overflow()`, with `DL_SCALE` from
//! `kernel/sched/sched.h`. The entity's parameters (relative deadline, runtime
//! and period) are passed in, as C takes them from `pi_of()`. The deferred
//! server flags and the yield flag are the scheduler's and are not modelled.

/// `DL_SCALE`: the overflow test works in units of 2^10 ns (about 1 us).
pub const DL_SCALE: u32 = 10;

/// The parameters a deadline task declared (`dl_deadline`, `dl_runtime`,
/// `dl_period`), in nanoseconds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DlParams {
    /// Relative deadline of each instance.
    pub dl_deadline: u64,
    /// Budget per instance.
    pub dl_runtime: i64,
    /// Period between instances.
    pub dl_period: u64,
}

/// The state of one deadline entity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DlEntity {
    /// Absolute deadline of the current instance.
    pub deadline: u64,
    /// Budget left in the current instance; may go negative on overrun.
    pub runtime: i64,
}

/// `dl_time_before()`, as in `kernel/sched/deadline.c`.
fn dl_time_before(a: u64, b: u64) -> bool {
    (a.wrapping_sub(b) as i64) < 0
}

/// `replenish_dl_new_period()`: start a new instance at `clock`.
pub fn replenish_dl_new_period(se: &mut DlEntity, p: &DlParams, clock: u64) {
    se.deadline = clock.wrapping_add(p.dl_deadline);
    se.runtime = p.dl_runtime;
}

/// `replenish_dl_entity()`: the budget is refilled whole periods at a time
/// until it is positive, moving the deadline with it; if the deadline is still
/// in the past, a new period starts at `clock`. A fresh entity (zero deadline)
/// or a deferred one that overflowed starts a new period straight away.
pub fn replenish_dl_entity(se: &mut DlEntity, p: &DlParams, clock: u64, deferred_overflow: bool) {
    if se.deadline == 0 || deferred_overflow {
        replenish_dl_new_period(se, p, clock);
    }
    while se.runtime <= 0 {
        se.deadline = se.deadline.wrapping_add(p.dl_period);
        se.runtime += p.dl_runtime;
    }
    if dl_time_before(se.deadline, clock) {
        replenish_dl_new_period(se, p, clock);
    }
}

/// `dl_entity_overflow()`: whether the remaining budget over the remaining time
/// exceeds the declared bandwidth. Compared by cross-multiplication, after
/// scaling both sides by `DL_SCALE` to keep the products in range.
pub fn dl_entity_overflow(se: &DlEntity, p: &DlParams, t: u64) -> bool {
    let left = (p.dl_deadline >> DL_SCALE) as i64 * (se.runtime >> DL_SCALE);
    let right = ((se.deadline.wrapping_sub(t)) >> DL_SCALE) as i64 * (p.dl_runtime >> DL_SCALE);
    right < left
}

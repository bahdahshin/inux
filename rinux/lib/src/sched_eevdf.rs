// SPDX-License-Identifier: GPL-2.0-only
//! EEVDF bookkeeping for the fair scheduler: the weighted average virtual
//! runtime, the eligibility test, and the deadline update.
//!
//! Ported from `kernel/sched/fair.c`: `avg_vruntime()`, `vruntime_eligible()`
//! and `update_deadline()`, over the `sum_w_vruntime` and `sum_weight`
//! accumulators of `struct cfs_rq`. C's `zero_vruntime` is advanced lazily
//! inside `avg_vruntime()`; here it is set by the caller, which keeps the
//! functions pure. The runqueue that owns the entities is the scheduler's.

use crate::sched_fair::{LoadWeight, calc_delta_fair};

/// The accumulators of a fair runqueue.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EevdfQueue {
    /// `sum_w_vruntime`: the sum of `weight * (vruntime - zero_vruntime)`.
    pub sum_w_vruntime: i128,
    /// `sum_weight`: the sum of the weights.
    pub sum_weight: u64,
    /// `zero_vruntime`: the reference point of the keys.
    pub zero_vruntime: u64,
}

/// `vruntime_op(a, "-", b)`: the signed difference, as C's `(s64)(a - b)`.
fn vruntime_sub(a: u64, b: u64) -> i64 {
    a.wrapping_sub(b) as i64
}

impl EevdfQueue {
    /// An empty queue whose keys are measured from `zero_vruntime`.
    pub const fn new(zero_vruntime: u64) -> Self {
        Self {
            sum_w_vruntime: 0,
            sum_weight: 0,
            zero_vruntime,
        }
    }

    /// Account an entity that joins the queue.
    pub fn enqueue(&mut self, vruntime: u64, weight: u64) {
        let key = vruntime_sub(vruntime, self.zero_vruntime);
        self.sum_w_vruntime += i128::from(key) * i128::from(weight);
        self.sum_weight += weight;
    }

    /// Account an entity that leaves the queue.
    pub fn dequeue(&mut self, vruntime: u64, weight: u64) {
        let key = vruntime_sub(vruntime, self.zero_vruntime);
        self.sum_w_vruntime -= i128::from(key) * i128::from(weight);
        self.sum_weight -= weight;
    }

    /// `avg_vruntime()`: the weighted average virtual runtime. With no
    /// entities, C returns the reference point.
    pub fn avg_vruntime(&self) -> u64 {
        if self.sum_weight == 0 {
            return self.zero_vruntime;
        }
        let w = i128::from(self.sum_weight);
        let mut runtime = self.sum_w_vruntime;
        // A negative sum rounds towards minus infinity, as C's comment says.
        if runtime < 0 {
            runtime -= w - 1;
        }
        let delta = (runtime / w) as i64;
        self.zero_vruntime.wrapping_add(delta as u64)
    }

    /// `vruntime_eligible()`: whether an entity at `vruntime` is at or below
    /// the weighted average, that is `sum_w_vruntime >= key * sum_weight`.
    pub fn vruntime_eligible(&self, vruntime: u64) -> bool {
        let key = vruntime_sub(vruntime, self.zero_vruntime);
        self.sum_w_vruntime >= i128::from(key) * i128::from(self.sum_weight)
    }
}

/// `update_deadline()`: once `vruntime` has reached the entity's deadline, the
/// deadline advances by one request, `slice / weight` in virtual time. Returns
/// whether the request was used up (so the caller reschedules) and the
/// deadline in force afterwards.
pub fn update_deadline(vruntime: u64, deadline: u64, slice: u64, weight: u64) -> (bool, u64) {
    if vruntime_sub(vruntime, deadline) < 0 {
        return (false, deadline);
    }
    let mut lw = LoadWeight {
        weight,
        inv_weight: 0,
    };
    (true, vruntime.wrapping_add(calc_delta_fair(slice, &mut lw)))
}

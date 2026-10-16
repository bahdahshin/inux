// SPDX-License-Identifier: GPL-2.0
//! CFS load weights: the nice-to-weight tables and the vruntime scaling
//! `delta * NICE_0_LOAD / weight` that the fair scheduler uses.
//!
//! Ported from `kernel/sched/core.c` (`sched_prio_to_weight[]` and
//! `sched_prio_to_wmult[]`) and `kernel/sched/fair.c` (`__update_inv_weight()`,
//! `__calc_delta()` and `calc_delta_fair()`), with the 64-bit `scale_load()`
//! convention of `kernel/sched/sched.h`. The two tables are copied from the C
//! source by a script, not retyped. Each `wmult` entry is `2^32 / weight`, which
//! turns the division in the delta into a multiplication, as C does.

/// `SCHED_FIXEDPOINT_SHIFT`: the extra precision of a scaled load weight.
pub const SCHED_FIXEDPOINT_SHIFT: u32 = 10;
/// `NICE_0_LOAD` on a 64-bit kernel: a nice-0 weight, scaled.
pub const NICE_0_LOAD: u64 = 1 << (SCHED_FIXEDPOINT_SHIFT * 2);
/// `WMULT_CONST`.
pub const WMULT_CONST: u32 = u32::MAX;
/// `WMULT_SHIFT`.
pub const WMULT_SHIFT: u32 = 32;

/// `scale_load()`: the scaled form of a weight.
pub const fn scale_load(w: u64) -> u64 {
    w << SCHED_FIXEDPOINT_SHIFT
}

/// `scale_load_down()`: back from the scaled form, never below 2 for a
/// non-zero weight.
pub fn scale_load_down(w: u64) -> u64 {
    if w != 0 {
        (w >> SCHED_FIXEDPOINT_SHIFT).max(2)
    } else {
        0
    }
}

/// `struct load_weight`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LoadWeight {
    /// The weight, in scaled form.
    pub weight: u64,
    /// `2^32 / weight`, computed on first use; zero until then.
    pub inv_weight: u32,
}

/// `sched_prio_to_weight[]`: the weight of each nice level, nice -20 first.
pub const SCHED_PRIO_TO_WEIGHT: [u32; 40] = [
          88761,     71755,     56483,     46273,     36291,
          29154,     23254,     18705,     14949,     11916,
           9548,      7620,      6100,      4904,      3906,
           3121,      2501,      1991,      1586,      1277,
           1024,       820,       655,       526,       423,
            335,       272,       215,       172,       137,
            110,        87,        70,        56,        45,
             36,        29,        23,        18,        15,
];

/// `sched_prio_to_wmult[]`: `2^32 / weight` for each nice level.
pub const SCHED_PRIO_TO_WMULT: [u32; 40] = [
          48388,     59856,     76040,     92818,    118348,
         147320,    184698,    229616,    287308,    360437,
         449829,    563644,    704093,    875809,   1099582,
        1376151,   1717300,   2157191,   2708050,   3363326,
        4194304,   5237765,   6557202,   8165337,  10153587,
       12820798,  15790321,  19976592,  24970740,  31350126,
       39045157,  49367440,  61356676,  76695844,  95443717,
      119304647, 148102320, 186737708, 238609294, 286331153,
];

/// The table index of a nice value in `-20..=19`, or `None` outside it.
pub fn nice_index(nice: i32) -> Option<usize> {
    (-20..=19).contains(&nice).then(|| (nice + 20) as usize)
}

/// `__update_inv_weight()`: fill in `inv_weight` from the weight, once.
fn update_inv_weight(lw: &mut LoadWeight) {
    if lw.inv_weight != 0 {
        return;
    }
    let w = scale_load_down(lw.weight);
    lw.inv_weight = if w >= u64::from(WMULT_CONST) {
        1
    } else if w == 0 {
        WMULT_CONST
    } else {
        (u64::from(WMULT_CONST) / w) as u32
    };
}

/// `__calc_delta()`: `delta_exec * weight / lw.weight`, computed as the
/// multiplication by `lw.inv_weight` and a shift, as C does.
pub fn calc_delta(delta_exec: u64, weight: u64, lw: &mut LoadWeight) -> u64 {
    let mut fact = scale_load_down(weight);
    let mut fact_hi = (fact >> 32) as u32;
    let mut shift = WMULT_SHIFT;

    update_inv_weight(lw);

    if fact_hi != 0 {
        let fs = 32 - fact_hi.leading_zeros();
        shift -= fs;
        fact >>= fs;
    }

    fact *= u64::from(lw.inv_weight);

    fact_hi = (fact >> 32) as u32;
    if fact_hi != 0 {
        let fs = 32 - fact_hi.leading_zeros();
        shift -= fs;
        fact >>= fs;
    }

    ((u128::from(delta_exec) * u128::from(fact)) >> shift) as u64
}

/// `calc_delta_fair()`: scale a runtime to nice-0 virtual time for an entity
/// whose load weight is `h_load`.
pub fn calc_delta_fair(delta: u64, h_load: &mut LoadWeight) -> u64 {
    if h_load.weight != NICE_0_LOAD {
        return calc_delta(delta, NICE_0_LOAD, h_load);
    }
    delta
}

// SPDX-License-Identifier: GPL-2.0-only
//! The load average: three exponentially decayed counts of runnable tasks
//! (1, 5 and 15 minutes), in fixed point.
//!
//! Ported from `kernel/sched/loadavg.c` (`calc_load()` as the header defines it,
//! `fixed_power_int()`, `calc_load_n()` and the `avenrun[]` update of
//! `calc_global_load()`) and `include/linux/sched/loadavg.h` (the constants and
//! `LOAD_INT`/`LOAD_FRAC`). The NO_HZ accounting, which folds task counts from
//! idle CPUs, is the scheduler's; this module takes the summed active count.

/// `FSHIFT`: bits of fraction in a load value.
pub const FSHIFT: u32 = 11;
/// `FIXED_1`: 1.0 in fixed point.
pub const FIXED_1: u64 = 1 << FSHIFT;
/// `EXP_1`: 1/exp(5s/1min) in fixed point.
pub const EXP_1: u64 = 1884;
/// `EXP_5`: 1/exp(5s/5min) in fixed point.
pub const EXP_5: u64 = 2014;
/// `EXP_15`: 1/exp(5s/15min) in fixed point.
pub const EXP_15: u64 = 2037;

/// `calc_load()`: one decay step, `load * exp + active * (1 - exp)`, rounded up
/// when the load is still climbing, as the header does.
pub fn calc_load(load: u64, exp: u64, active: u64) -> u64 {
    let mut newload = load * exp + active * (FIXED_1 - exp);
    if active >= load {
        newload += FIXED_1 - 1;
    }
    newload / FIXED_1
}

/// `fixed_power_int()`: `x^n` in fixed point with `frac_bits` of fraction,
/// by repeated squaring with rounding, as C does.
pub fn fixed_power_int(x: u64, frac_bits: u32, n: u32) -> u64 {
    let half = 1u64 << (frac_bits - 1);
    let mut result = 1u64 << frac_bits;
    let mut x = x;
    let mut n = n;
    if n != 0 {
        loop {
            if n & 1 != 0 {
                result = (result * x + half) >> frac_bits;
            }
            n >>= 1;
            if n == 0 {
                break;
            }
            x = (x * x + half) >> frac_bits;
        }
    }
    result
}

/// `calc_load_n()`: `n` steps of decay at once.
pub fn calc_load_n(load: u64, exp: u64, active: u64, n: u32) -> u64 {
    calc_load(load, fixed_power_int(exp, FSHIFT, n), active)
}

/// The integer part of a load value (`LOAD_INT`).
pub const fn load_int(x: u64) -> u64 {
    x >> FSHIFT
}

/// The two-digit fraction of a load value (`LOAD_FRAC`).
pub const fn load_frac(x: u64) -> u64 {
    load_int((x & (FIXED_1 - 1)) * 100)
}

/// The `avenrun[]` update of `calc_global_load()`: decay the three averages
/// towards the number of active tasks. `active` is the summed count, which is
/// clamped at zero.
pub fn update_avenrun(avenrun: &mut [u64; 3], active: i64) {
    let active = if active > 0 {
        active as u64 * FIXED_1
    } else {
        0
    };
    avenrun[0] = calc_load(avenrun[0], EXP_1, active);
    avenrun[1] = calc_load(avenrun[1], EXP_5, active);
    avenrun[2] = calc_load(avenrun[2], EXP_15, active);
}

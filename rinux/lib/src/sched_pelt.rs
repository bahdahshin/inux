// SPDX-License-Identifier: GPL-2.0-only
//! Per-entity load tracking (PELT): the geometric decay that makes old load
//! count for less, with y^32 = 1/2.
//!
//! Ported from `kernel/sched/pelt.c`, `decay_load()` and
//! `__accumulate_pelt_segments()`, and the constants of
//! `kernel/sched/sched-pelt.h`. The inverse-decay table is copied from that
//! header by script. `decay_load(v, n)` is `v * y^n`: whole periods of 32 halve
//! the value, and the remainder is one table lookup, so the cost does not grow
//! with `n`.

/// `LOAD_AVG_PERIOD`: the length of one decay period, in 1024 us units.
pub const LOAD_AVG_PERIOD: u64 = 32;
/// `LOAD_AVG_MAX`: the sum of the decayed series, `1024 * sum y^n`.
pub const LOAD_AVG_MAX: u64 = 47742;

/// `runnable_avg_yN_inv[]`: `2^32 * y^n` for `n` in one period.
pub const RUNNABLE_AVG_YN_INV: [u32; 32] = [
    0xffffffff, 0xfa83b2da, 0xf5257d14, 0xefe4b99a, 0xeac0c6e6, 0xe5b906e6, 0xe0ccdeeb, 0xdbfbb796,
    0xd744fcc9, 0xd2a81d91, 0xce248c14, 0xc9b9bd85, 0xc5672a10, 0xc12c4cc9, 0xbd08a39e, 0xb8fbaf46,
    0xb504f333, 0xb123f581, 0xad583ee9, 0xa9a15ab4, 0xa5fed6a9, 0xa2704302, 0x9ef5325f, 0x9b8d39b9,
    0x9837f050, 0x94f4efa8, 0x91c3d373, 0x8ea4398a, 0x8b95c1e3, 0x88980e80, 0x85aac367, 0x82cd8698,
];

/// `decay_load()`: `val * y^n`, with y^32 = 1/2. Beyond `LOAD_AVG_PERIOD * 63`
/// periods the value is gone, as C returns 0 there.
pub fn decay_load(val: u64, n: u64) -> u64 {
    if n > LOAD_AVG_PERIOD * 63 {
        return 0;
    }
    let mut val = val;
    let mut local_n = n;
    if local_n >= LOAD_AVG_PERIOD {
        val >>= local_n / LOAD_AVG_PERIOD;
        local_n %= LOAD_AVG_PERIOD;
    }
    ((u128::from(val) * u128::from(RUNNABLE_AVG_YN_INV[local_n as usize])) >> 32) as u64
}

/// `__accumulate_pelt_segments()`: the decayed contribution of a span made of
/// the remainder `d1` of the last period, `periods` whole periods, and the
/// current period's `d3`.
pub fn accumulate_pelt_segments(periods: u64, d1: u32, d3: u32) -> u32 {
    assert!(periods >= 1, "PELT accumulates at least one period");
    let c1 = decay_load(u64::from(d1), periods);
    // 1024 * sum_{n=1}^{periods-1} y^n
    let c2 = LOAD_AVG_MAX - decay_load(LOAD_AVG_MAX, periods) - 1024;
    (c1 + c2 + u64::from(d3)) as u32
}

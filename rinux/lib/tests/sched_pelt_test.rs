// SPDX-License-Identifier: GPL-2.0-or-later
//! Host tests for PELT's decay: the table against y^n, whole periods halving,
//! and the series constant LOAD_AVG_MAX. Linux has no KUnit file for this.

use rinux_lib::sched_pelt::*;

fn y() -> f64 {
    0.5f64.powf(1.0 / 32.0)
}

#[test]
fn decay_matches_the_real_exponential() {
    for n in 0..300u64 {
        let v = 1_000_000u64;
        let want = v as f64 * y().powi(n as i32);
        let got = decay_load(v, n) as f64;
        assert!(
            (got - want).abs() <= want * 1e-6 + 2.0,
            "n={n}: {got} vs {want}"
        );
    }
}

#[test]
fn a_whole_period_halves_to_within_one() {
    // The table's entry 0 is 0xffffffff, i.e. 1 - 2^-32, so a multiply by it
    // loses one unit, as in C.
    let a = decay_load(1 << 20, 32);
    assert!(a.abs_diff(1 << 19) <= 1, "{a}");
    let b = decay_load(1 << 20, 64);
    assert!(b.abs_diff(1 << 18) <= 1, "{b}");
}

#[test]
fn very_old_load_is_gone() {
    assert_eq!(decay_load(u64::MAX, LOAD_AVG_PERIOD * 63 + 1), 0);
}

#[test]
fn one_period_of_segments_is_small() {
    // With no current or partial load, one period adds only the decayed tail
    // of the series, which is well under one full period's weight.
    assert!(accumulate_pelt_segments(1, 0, 0) < 1024);
}

#[test]
fn table_entry_zero_is_the_identity_to_within_one() {
    assert_eq!(RUNNABLE_AVG_YN_INV[0], u32::MAX);
}

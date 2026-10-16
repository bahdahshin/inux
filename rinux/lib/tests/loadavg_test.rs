// SPDX-License-Identifier: GPL-2.0-or-later
//! Host tests for the load average: the fixed-point decay against the real
//! exponential, convergence to the active count, and the fraction format.
//! Linux has no KUnit file for loadavg.c.

use rinux_lib::loadavg::*;

#[test]
fn constants_match_the_header() {
    assert_eq!(FIXED_1, 2048);
    assert_eq!((EXP_1, EXP_5, EXP_15), (1884, 2014, 2037));
}

#[test]
fn decay_tracks_the_real_exponential() {
    // exp(-5/60) = 0.92, the one-minute factor per 5-second sample.
    let real = (-5.0f64 / 60.0).exp();
    let fixed = EXP_1 as f64 / FIXED_1 as f64;
    assert!((fixed - real).abs() < 0.001, "{fixed} vs {real}");
}

#[test]
fn a_constant_load_converges_to_the_active_count() {
    let mut avg = [0u64; 3];
    for _ in 0..2000 {
        update_avenrun(&mut avg, 3);
    }
    for v in avg {
        assert!(v.abs_diff(3 * FIXED_1) <= 4, "{v}");
    }
}

#[test]
fn the_one_minute_average_moves_faster_than_fifteen() {
    let mut avg = [0u64; 3];
    for _ in 0..24 {
        update_avenrun(&mut avg, 2);
    }
    assert!(avg[0] > avg[1] && avg[1] > avg[2]);
}

#[test]
fn negative_active_counts_clamp_to_zero() {
    let mut avg = [4 * FIXED_1; 3];
    update_avenrun(&mut avg, -7);
    assert!(avg[0] < 4 * FIXED_1);
}

#[test]
fn fixed_power_matches_repeated_multiplication() {
    let x = EXP_1;
    let p = fixed_power_int(x, FSHIFT, 12) as f64 / FIXED_1 as f64;
    let want = (EXP_1 as f64 / FIXED_1 as f64).powi(12);
    assert!((p - want).abs() < 0.001, "{p} vs {want}");
}

#[test]
fn load_int_and_frac_split_a_value() {
    let x = FIXED_1 + FIXED_1 / 2; // 1.50
    assert_eq!(load_int(x), 1);
    assert_eq!(load_frac(x), 50);
}

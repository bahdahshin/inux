// SPDX-License-Identifier: GPL-2.0-or-later
//! Host tests for the CFS load-weight tables and the vruntime scaling. Linux
//! has no KUnit file for these; they are checked against the exact arithmetic.

use rinux_lib::sched_fair::*;

#[test]
fn nice_zero_is_1024_and_its_inverse_is_2_pow_22() {
    let i = nice_index(0).unwrap();
    assert_eq!(SCHED_PRIO_TO_WEIGHT[i], 1024);
    assert_eq!(SCHED_PRIO_TO_WMULT[i], 4_194_304);
    assert_eq!(nice_index(-21), None);
    assert_eq!(nice_index(20), None);
}

#[test]
fn each_inverse_is_two_to_the_32_over_its_weight() {
    for i in 0..40 {
        let w = u64::from(SCHED_PRIO_TO_WEIGHT[i]);
        let inv = u64::from(SCHED_PRIO_TO_WMULT[i]);
        let exact = (1u64 << 32) / w;
        let diff = inv.abs_diff(exact);
        assert!(diff <= 1, "nice {}: {inv} vs {exact}", i as i32 - 20);
    }
}

#[test]
fn weights_fall_as_nice_rises() {
    assert!(SCHED_PRIO_TO_WEIGHT.windows(2).all(|w| w[0] > w[1]));
}

#[test]
fn nice_zero_entity_is_unscaled() {
    let mut lw = LoadWeight {
        weight: NICE_0_LOAD,
        inv_weight: 0,
    };
    assert_eq!(calc_delta_fair(123_456, &mut lw), 123_456);
}

#[test]
fn heavier_entities_accrue_less_virtual_time() {
    // nice -5 weighs 3121, nice 0 weighs 1024; the vruntime delta is about
    // delta * 1024 / 3121 when both are scaled.
    let heavy = scale_load(u64::from(SCHED_PRIO_TO_WEIGHT[nice_index(-5).unwrap()]));
    let mut h = LoadWeight {
        weight: heavy,
        inv_weight: 0,
    };
    let delta = 10_000_000u64;
    let got = calc_delta_fair(delta, &mut h);
    let want = (u128::from(delta) * 1024 / 3121) as u64;
    let err = got.abs_diff(want);
    assert!(err * 1000 <= want, "got {got}, want {want}");
}

#[test]
fn scale_load_down_keeps_a_minimum_of_two() {
    assert_eq!(scale_load_down(0), 0);
    assert_eq!(scale_load_down(1), 2);
    assert_eq!(scale_load_down(scale_load(1024)), 1024);
}

// SPDX-License-Identifier: GPL-2.0-or-later
//! Host tests for the task-to-cpupri level mapping. Linux has no KUnit file
//! for it.

use rinux_lib::cpupri::*;

#[test]
fn the_special_levels_map_to_themselves() {
    assert_eq!(convert_prio(CPUPRI_INVALID), Some(CPUPRI_INVALID));
    assert_eq!(convert_prio(99), Some(CPUPRI_NORMAL));
    assert_eq!(convert_prio(100), Some(CPUPRI_HIGHER));
}

#[test]
fn real_time_priorities_are_reversed_into_levels_one_to_ninety_nine() {
    assert_eq!(convert_prio(0), Some(99));
    assert_eq!(convert_prio(98), Some(1));
}

#[test]
fn a_more_urgent_task_gets_a_higher_level() {
    let levels: Vec<i32> = (0..=98).map(|p| convert_prio(p).unwrap()).collect();
    assert!(levels.windows(2).all(|w| w[0] > w[1]));
}

#[test]
fn priorities_outside_the_scale_are_refused() {
    assert_eq!(convert_prio(-2), None);
    assert_eq!(convert_prio(101), None);
    assert!(CPUPRI_NR_PRIORITIES == 101);
}

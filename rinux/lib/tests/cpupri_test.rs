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

#[test]
fn set_moves_cpus_between_levels() {
    use rinux_lib::cpupri::CpuPri;
    let mut cp: CpuPri<4> = CpuPri::new();
    assert_eq!(cp.level(0), CPUPRI_INVALID);
    cp.set(0, 50); // a normal-range task: level 49
    cp.set(1, 99); // ordinary: level 0
    assert_eq!(cp.level(0), 49);
    cp.set(0, 99);
    assert_eq!(cp.level(0), CPUPRI_NORMAL);
}

#[test]
fn find_picks_a_lower_level_cpu_the_task_may_use() {
    use rinux_lib::cpupri::CpuPri;
    let mut cp: CpuPri<4> = CpuPri::new();
    cp.set(0, 99); // level 0
    cp.set(1, 20); // level 79
    let all = [true; 4];
    // A task at priority 10 (level 89) can use any CPU below it: CPU 0 first.
    assert_eq!(cp.find(10, &all), Some(0));
    // With CPU 0 disallowed, CPU 1 is the next choice.
    assert_eq!(cp.find(10, &[false, true, true, true]), Some(1));
    // A task at normal level cannot displace anything.
    assert_eq!(cp.find(99, &all), None);
}

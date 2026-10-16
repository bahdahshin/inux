// SPDX-License-Identifier: GPL-2.0-or-later
//! Host tests for the scheduler priority numbers and `normal_prio`. Linux has
//! no KUnit file for these.

use rinux_lib::sched_prio::*;

#[test]
fn ranges_match_the_header() {
    assert_eq!(NICE_WIDTH, 40);
    assert_eq!(MAX_PRIO, 140);
    assert_eq!(DEFAULT_PRIO, 120);
    assert_eq!(nice_to_prio(0), 120);
    assert_eq!(prio_to_nice(nice_to_prio(-20)), -20);
}

#[test]
fn ordinary_tasks_map_their_nice_level() {
    assert_eq!(normal_prio(0, 0, -20), 100);
    assert_eq!(normal_prio(0, 0, 19), 139);
}

#[test]
fn real_time_priority_one_is_just_below_the_top() {
    // rt_prio 1 is the lowest real-time priority; 99 the highest.
    assert_eq!(normal_prio(SCHED_FIFO, 1, 0), 98);
    assert_eq!(normal_prio(SCHED_RR, 99, 0), 0);
}

#[test]
fn deadline_tasks_outrank_real_time() {
    assert_eq!(normal_prio(SCHED_DEADLINE, 0, 0), -1);
    assert!(normal_prio(SCHED_DEADLINE, 0, 0) < normal_prio(SCHED_FIFO, 99, 0));
}

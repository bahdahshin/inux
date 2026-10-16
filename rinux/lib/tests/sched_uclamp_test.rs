// SPDX-License-Identifier: GPL-2.0-or-later
//! Host tests for the utilization clamp bucket mapping. Linux has no KUnit file
//! for it.

use rinux_lib::sched_uclamp::*;

#[test]
fn bucket_width_is_capacity_over_the_bucket_count_rounded() {
    assert_eq!(UCLAMP_BUCKET_DELTA, 205); // 1024 / 5 = 204.8, rounded up
    assert_eq!(div_round_closest(7, 2), 4);
    assert_eq!(div_round_closest(6, 4), 2);
}

#[test]
fn boundaries_fall_on_bucket_edges() {
    assert_eq!(uclamp_bucket_id(0), 0);
    assert_eq!(uclamp_bucket_id(204), 0);
    assert_eq!(uclamp_bucket_id(205), 1);
    assert_eq!(uclamp_bucket_id(409), 1);
    assert_eq!(uclamp_bucket_id(410), 2);
}

#[test]
fn the_top_bucket_takes_everything_above_its_edge() {
    assert_eq!(uclamp_bucket_id(820), 4);
    assert_eq!(uclamp_bucket_id(1023), 4);
    assert_eq!(uclamp_bucket_id(1024), 4);
    assert_eq!(uclamp_bucket_id(u32::MAX), UCLAMP_BUCKETS - 1);
}

#[test]
fn the_mapping_is_monotone_and_in_range() {
    let mut last = 0;
    for v in 0..=SCHED_CAPACITY_SCALE {
        let id = uclamp_bucket_id(v);
        assert!(id < UCLAMP_BUCKETS);
        assert!(id >= last);
        last = id;
    }
}

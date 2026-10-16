// SPDX-License-Identifier: GPL-2.0 OR Linux-OpenIB
//! Host tests for the DIM port. Linux has no KUnit file for `lib/dim/`, so
//! these check the arithmetic by hand and walk the RDMA state machine through
//! the windows that the C comments describe.

use rinux_lib::dim::*;

/// One millisecond, in the nanoseconds `ktime_t` uses.
const MS: u64 = 1_000_000;

#[test]
fn significant_diff_needs_more_than_ten_percent() {
    assert!(!is_significant_diff(110, 100));
    assert!(is_significant_diff(111, 100));
    assert!(is_significant_diff(89, 100));
    assert!(!is_significant_diff(90, 100));
    // A zero reference is never significant, whatever the value.
    assert!(!is_significant_diff(1_000, 0));
}

#[test]
fn bit_gap_wraps_at_32_bits() {
    assert_eq!(bit_gap32(10, 4), 6);
    assert_eq!(bit_gap32(3, u32::MAX - 1), 5);
}

#[test]
fn calc_stats_by_hand() {
    // 1 ms apart: every per-ms rate is the raw count, and 64 events give 64 ev/ms.
    let start = DimSample {
        time: 0,
        pkt_ctr: 0,
        byte_ctr: 0,
        event_ctr: 0,
        comp_ctr: 0,
    };
    let end = DimSample {
        time: MS,
        pkt_ctr: 100,
        byte_ctr: 2500,
        event_ctr: 64,
        comp_ctr: 50,
    };
    let stats = dim_calc_stats(&start, &end).unwrap();
    assert_eq!(stats.ppms, 100);
    assert_eq!(stats.bpms, 2500);
    assert_eq!(stats.epms, 64);
    assert_eq!(stats.cpms, 50);
    // 50 completions per 64 events, times 100, rounded down: 78.125 -> 78.
    assert_eq!(stats.cpe_ratio, 78);
}

#[test]
fn calc_stats_rounds_rates_up() {
    // 1.5 ms: 100 packets is 66.67 per ms, which rounds up to 67.
    let start = DimSample::default();
    let end = DimSample {
        time: 1_500_000,
        pkt_ctr: 100,
        ..DimSample::default()
    };
    let stats = dim_calc_stats(&start, &end).unwrap();
    assert_eq!(stats.ppms, 67);
    // 64 events in 1.5 ms is 42.67 per ms, which rounds up to 43.
    assert_eq!(stats.epms, 43);
}

#[test]
fn calc_stats_without_time_passing_is_none() {
    let s = DimSample {
        time: 5 * MS,
        pkt_ctr: 1,
        ..DimSample::default()
    };
    assert_eq!(dim_calc_stats(&s, &s), None);
}

#[test]
fn calc_stats_handles_counter_wrap() {
    let start = DimSample {
        time: 0,
        pkt_ctr: u32::MAX - 9,
        ..DimSample::default()
    };
    let end = DimSample {
        time: MS,
        pkt_ctr: 10,
        ..DimSample::default()
    };
    // 20 packets across the wrap, in 1 ms.
    assert_eq!(dim_calc_stats(&start, &end).unwrap().ppms, 20);
}

#[test]
fn sample_update_sets_time_and_truncates_counters() {
    let mut s = DimSample::default();
    update_sample_with_comps(7 * MS, 9, 1 << 33 | 5, 6, 1 << 32 | 3, &mut s);
    assert_eq!(s.time, 7 * MS);
    assert_eq!(s.event_ctr, 9);
    assert_eq!(s.pkt_ctr, 5);
    assert_eq!(s.byte_ctr, 6);
    assert_eq!(s.comp_ctr, 3);
}

#[test]
fn turn_and_park_follow_the_walk() {
    let mut dim = Dim::new();
    dim.steps_right = 3;
    dim.steps_left = 2;
    dim_turn(&mut dim);
    assert_eq!(dim.tune_state, DimTuneState::GoingLeft);
    assert_eq!(dim.steps_left, 0);
    assert_eq!(dim.steps_right, 3);
    dim_turn(&mut dim);
    assert_eq!(dim.tune_state, DimTuneState::GoingRight);
    assert_eq!(dim.steps_right, 0);

    dim.tired = 4;
    dim_park_on_top(&mut dim);
    assert_eq!(dim.tune_state, DimTuneState::ParkingOnTop);
    assert_eq!(dim.tired, 0);
    // Turning a parked instance does nothing.
    dim_turn(&mut dim);
    assert_eq!(dim.tune_state, DimTuneState::ParkingOnTop);

    dim_park_tired(&mut dim);
    assert_eq!(dim.tune_state, DimTuneState::ParkingTired);
    assert!(dim_on_top(&dim));
}

#[test]
fn on_top_needs_the_other_side_to_have_been_tried() {
    let mut dim = Dim::new();
    dim.steps_left = 2;
    dim.steps_right = 1;
    assert!(dim_on_top(&dim));
    dim.steps_left = 1;
    assert!(!dim_on_top(&dim));

    dim.tune_state = DimTuneState::GoingLeft;
    dim.steps_right = 2;
    dim.steps_left = 1;
    assert!(dim_on_top(&dim));
}

#[test]
fn rdma_dim_moves_up_when_completions_rise() {
    let mut dim = Dim::new();
    // The first call starts a measurement; events 1..=65 end the first window.
    // One completion per event: 1 per ms, 100 % per event, no profile change.
    let mut results = vec![dim.rdma_dim(MS, 0)];
    assert_eq!(dim.state, DimState::MeasureInProgress);
    for n in 2..=65u32 {
        results.push(dim.rdma_dim(n as u64 * MS, 1));
    }
    assert!(results.iter().all(|&r| !r));
    assert_eq!(dim.profile_ix, 0);
    assert_eq!(dim.prev_stats.cpms, 1);
    assert_eq!(dim.prev_stats.cpe_ratio, 100);

    // The next window doubles the completion rate: 2 per ms is "better" by
    // more than 10 %, so the walk moves up one profile and asks for the work.
    let mut scheduled = false;
    for n in 66..=129u32 {
        scheduled = dim.rdma_dim(n as u64 * MS, 2);
        if scheduled {
            assert_eq!(n, 129, "a decision needs 64 events in the window");
            break;
        }
    }
    assert!(scheduled);
    assert_eq!(dim.profile_ix, 1);
    assert_eq!(dim.steps_right, 1);
    assert_eq!(dim.state, DimState::ApplyNewProfile);
    assert_eq!(dim.tune_state, DimTuneState::GoingRight);

    // Nothing happens while the consumer applies the profile.
    assert!(!dim.rdma_dim(200 * MS, 2));
    assert_eq!(dim.state, DimState::ApplyNewProfile);
}

#[test]
fn rdma_dim_turns_at_the_top_profile() {
    let mut dim = Dim::new();
    dim.profile_ix = RDMA_DIM_PARAMS_NUM_PROFILES - 1;
    dim.state = DimState::MeasureInProgress;
    dim.start_sample = DimSample {
        time: 0,
        event_ctr: 0,
        comp_ctr: 0,
        ..DimSample::default()
    };
    dim.prev_stats = DimStats {
        cpms: 1,
        cpe_ratio: 100,
        ..DimStats::default()
    };
    // A better window at the last profile: no step is possible, so the walk
    // turns to the left and the profile is kept.
    dim.measuring_sample = DimSample {
        event_ctr: 63,
        comp_ctr: 0,
        ..DimSample::default()
    };
    let scheduled = dim.rdma_dim(64 * MS, 200);
    assert!(!scheduled);
    assert_eq!(dim.tune_state, DimTuneState::GoingLeft);
    assert_eq!(dim.profile_ix, RDMA_DIM_PARAMS_NUM_PROFILES - 1);
}

#[test]
fn rdma_dim_keeps_the_start_sample_before_enough_events() {
    let mut dim = Dim::new();
    dim.state = DimState::MeasureInProgress;
    dim.start_sample = DimSample {
        time: MS,
        event_ctr: 10,
        comp_ctr: 4,
        ..DimSample::default()
    };
    dim.measuring_sample = DimSample {
        event_ctr: 9,
        comp_ctr: 4,
        ..DimSample::default()
    };
    // The tenth event arrives: ten events since the start, so not yet 64.
    assert!(!dim.rdma_dim(20 * MS, 1));
    assert_eq!(dim.start_sample.event_ctr, 10);
    assert_eq!(dim.start_sample.time, MS);
    assert_eq!(dim.state, DimState::MeasureInProgress);
}

#[test]
fn rdma_dim_event_counter_wrap_counts_as_many_events() {
    // C computes `nevents` from two u16 counters as `int`, then stores it in
    // a u32. A counter that wrapped past the start gives a huge difference,
    // so the window is treated as complete. The port keeps that behaviour.
    let mut dim = Dim::new();
    dim.state = DimState::MeasureInProgress;
    dim.start_sample = DimSample {
        time: 0,
        event_ctr: 65_530,
        ..DimSample::default()
    };
    dim.measuring_sample = DimSample {
        event_ctr: 2,
        ..DimSample::default()
    };
    assert!(!dim.rdma_dim(MS, 0));
    // The window was closed and a new one started at the current event.
    assert_eq!(dim.start_sample.event_ctr, 3);
    assert_eq!(dim.state, DimState::MeasureInProgress);
}

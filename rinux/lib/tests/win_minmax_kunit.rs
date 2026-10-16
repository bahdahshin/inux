// SPDX-License-Identifier: GPL-2.0
//! Tests for the windowed min/max tracker. The first two use hand-computed
//! sequences where Nichols' algorithm is exact; the last cross-checks against
//! a brute-force windowed max on a run where the algorithm is also exact
//! (the window is large enough that no best sample ever expires).

use rinux_lib::win_minmax::Minmax;

#[test]
fn running_max_drops_when_peak_leaves_window() {
    // win=4; a peak of 5 at t=0 should hold until it ages out at t=5.
    let mut m = Minmax::new();
    let seq = [(0u32, 5u32), (1, 3), (2, 4), (3, 2), (5, 1)];
    let got: Vec<u32> = seq.iter().map(|&(t, v)| m.running_max(4, t, v)).collect();
    assert_eq!(got, vec![5, 5, 5, 5, 4]);
}

#[test]
fn running_min_rises_when_trough_leaves_window() {
    // A min tracker must be reset by the first measurement (a zeroed struct's
    // min is 0, which nothing beats), then it tracks as usual.
    let mut m = Minmax::new();
    let mut got = vec![m.reset(0, 5)];
    for &(t, v) in &[(1u32, 7u32), (2, 6), (3, 8), (5, 9)] {
        got.push(m.running_min(4, t, v));
    }
    assert_eq!(got, vec![5, 5, 5, 5, 6]);
}

#[test]
fn monotonic_returns_latest() {
    // Each new sample is a new max, so the result tracks the input exactly.
    let mut mx = Minmax::new();
    for t in 0..20u32 {
        assert_eq!(mx.running_max(100, t, t * 3 + 1), t * 3 + 1);
    }
    // A strictly decreasing stream is a new min each step (after reset).
    let mut mn = Minmax::new();
    assert_eq!(mn.reset(0, 1000), 1000);
    for t in 1..20u32 {
        assert_eq!(mn.running_min(100, t, 1000 - t), 1000 - t);
    }
}

#[test]
fn matches_bruteforce_within_large_window() {
    // With a window larger than the whole run, no sample ever expires, so the
    // tracker's max always equals the running maximum of all samples so far.
    let mut m = Minmax::new();
    let mut state = 12345u32;
    let mut best = 0u32;
    for t in 0..500u32 {
        state = state.wrapping_mul(1103515245).wrapping_add(12345);
        let v = state % 1000;
        best = best.max(v);
        let got = m.running_max(10_000, t, v);
        assert_eq!(got, best, "at t={t}, v={v}");
    }
}

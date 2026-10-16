// SPDX-License-Identifier: GPL-2.0-or-later
//! Host tests for `cputime_adjust`: the total is preserved, both parts are
//! monotonic over a run of samples, and the tick ratio is followed. Linux has
//! no KUnit file for this.

use rinux_lib::cputime::{CputimeSample, PrevCputime, cputime_adjust};

#[test]
fn the_parts_always_sum_to_the_precise_total() {
    let mut prev = PrevCputime::default();
    let s = CputimeSample {
        utime: 300,
        stime: 100,
        sum_exec_runtime: 1000,
    };
    let (u, st) = cputime_adjust(s, &mut prev);
    assert_eq!(u + st, 1000);
    // The ratio of ticks is 3:1, so the split follows it.
    assert_eq!(st, 250);
}

#[test]
fn no_system_ticks_means_all_user_time() {
    let mut prev = PrevCputime::default();
    let s = CputimeSample {
        utime: 5,
        stime: 0,
        sum_exec_runtime: 77,
    };
    assert_eq!(cputime_adjust(s, &mut prev), (77, 0));
}

#[test]
fn parts_never_go_backwards_over_a_run_of_samples() {
    let mut prev = PrevCputime::default();
    let mut last = (0u64, 0u64);
    let mut rtime = 0u64;
    let mut seed: u64 = 9;
    for step in 0..500u64 {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        rtime += 1 + (seed >> 60);
        // Ticks drift: sometimes system time jumps, sometimes it lags.
        let stime = (step * 3) % 97;
        let utime = (step * 7) % 113 + 1;
        let (u, st) = cputime_adjust(
            CputimeSample {
                utime,
                stime,
                sum_exec_runtime: rtime,
            },
            &mut prev,
        );
        assert!(u >= last.0 && st >= last.1, "step {step}");
        assert_eq!(u + st, rtime, "step {step}");
        last = (u, st);
    }
}

#[test]
fn a_non_monotonic_total_is_ignored() {
    let mut prev = PrevCputime {
        utime: 60,
        stime: 40,
    };
    let s = CputimeSample {
        utime: 1,
        stime: 1,
        sum_exec_runtime: 90,
    };
    assert_eq!(cputime_adjust(s, &mut prev), (60, 40));
}

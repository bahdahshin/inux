// SPDX-License-Identifier: GPL-2.0-or-later
//! Host tests for completions: counting, saturation, complete_all staying in
//! force, and the non-blocking wait. Linux has no KUnit file for completion.rs.

use rinux_lib::completion::Completion;

#[test]
fn each_complete_lets_one_waiter_through() {
    let c = Completion::new();
    assert!(!c.is_done());
    assert!(!c.try_wait());
    c.complete();
    c.complete();
    assert!(c.is_done());
    assert!(c.try_wait());
    assert!(c.try_wait());
    assert!(!c.try_wait());
}

#[test]
fn complete_all_is_never_used_up() {
    let c = Completion::new();
    c.complete_all();
    for _ in 0..5 {
        assert!(c.try_wait());
    }
    assert!(c.is_done());
    // A later complete() does not change the all-done state.
    c.complete();
    assert!(c.try_wait());
}

#[test]
fn concurrent_completes_are_all_counted() {
    let c = Completion::new();
    std::thread::scope(|s| {
        for _ in 0..4 {
            s.spawn(|| {
                for _ in 0..250 {
                    c.complete();
                }
            });
        }
    });
    let mut taken = 0;
    while c.try_wait() {
        taken += 1;
    }
    assert_eq!(taken, 1000);
}

// SPDX-License-Identifier: GPL-2.0-only
//! Host tests for `lib/rcuref.c`. Linux has no KUnit file for it, so these
//! walk the zones described in the C comment and check each transition.

use rinux_lib::rcuref::{RCUREF_ONEREF, RCUREF_RELEASED, RcuRef};
use std::sync::Arc;

#[test]
fn init_counts_references_from_one() {
    let r = RcuRef::new(1);
    assert_eq!(r.read(), 1);
    assert!(!r.is_dead());
    assert_eq!(RcuRef::new(3).read(), 3);
}

#[test]
fn get_and_put_balance_and_the_last_put_is_reported() {
    let r = RcuRef::new(1);
    assert!(r.get());
    assert_eq!(r.read(), 2);
    assert!(!r.put(), "not the last reference");
    assert_eq!(r.read(), 1);
    assert!(r.put(), "the last reference");
    assert_eq!(r.read(), 0);
    assert!(r.is_dead());
}

#[test]
fn released_zone_starts_at_the_constant() {
    assert_eq!(RCUREF_ONEREF, 0);
    assert_eq!(RCUREF_RELEASED, 0xC000_0000);
}

#[test]
fn threads_balancing_get_and_put_leave_the_count_intact() {
    let r = Arc::new(RcuRef::new(1));
    let handles: Vec<_> = (0..4)
        .map(|_| {
            let r = Arc::clone(&r);
            std::thread::spawn(move || {
                for _ in 0..10_000 {
                    assert!(r.get());
                    assert!(!r.put());
                }
            })
        })
        .collect();
    for h in handles {
        h.join().unwrap();
    }

    assert_eq!(r.read(), 1);
    assert!(r.put());
}

#[test]
fn exactly_one_thread_wins_the_last_put() {
    let r = Arc::new(RcuRef::new(4));
    let handles: Vec<_> = (0..4)
        .map(|_| {
            let r = Arc::clone(&r);
            std::thread::spawn(move || r.put())
        })
        .collect();
    let wins = handles
        .into_iter()
        .map(|h| h.join().unwrap())
        .filter(|&won| won)
        .count();
    assert_eq!(wins, 1);
    assert!(r.is_dead());
}

// SPDX-License-Identifier: GPL-2.0-or-later
//! Host tests for `lockref`. Linux has no KUnit file for it; these cover the
//! fast and slow paths, the dead state, and concurrent counting on threads.

use rinux_lib::lockref::{LOCKREF_DEAD_VAL, Lockref, PutOrLock};
use std::sync::Arc;

#[test]
fn get_and_put_move_the_count() {
    let r = Lockref::new(0);
    for _ in 0..5 {
        r.get();
    }
    assert_eq!(r.count(), 5);
    assert_eq!(r.put_return(), 4);
    assert_eq!(r.count(), 4);
}

#[test]
fn put_return_gives_up_at_zero_as_c_does() {
    let r = Lockref::new(0);
    assert_eq!(r.put_return(), -1);
    assert_eq!(r.count(), 0);
}

#[test]
fn get_not_zero_refuses_a_zero_count() {
    let r = Lockref::new(0);
    assert!(!r.get_not_zero());
    assert_eq!(r.count(), 0);

    let one = Lockref::new(1);
    assert!(one.get_not_zero());
    assert_eq!(one.count(), 2);
}

#[test]
fn put_or_lock_leaves_the_lock_held_for_the_last_reference() {
    let r = Lockref::new(2);
    assert_eq!(r.put_or_lock(), PutOrLock::Put);
    assert_eq!(r.count(), 1);

    assert_eq!(r.put_or_lock(), PutOrLock::LockHeld);
    assert_eq!(r.count(), 1);
    // The lock is held, so the fast path cannot run; release it first.
    r.unlock();
    assert!(r.get_not_zero());
}

#[test]
fn dead_objects_refuse_new_references() {
    let r = Lockref::new(3);
    r.lock();
    r.mark_dead();
    r.unlock();
    assert_eq!(r.count(), LOCKREF_DEAD_VAL);
    assert!(!r.get_not_dead());
    assert!(!r.get_not_zero());
    assert_eq!(r.put_return(), -1);
}

#[test]
fn concurrent_gets_are_all_counted() {
    let r = Arc::new(Lockref::new(0));
    let threads: Vec<_> = (0..8)
        .map(|_| {
            let r = Arc::clone(&r);
            std::thread::spawn(move || {
                for _ in 0..1000 {
                    r.get();
                }
            })
        })
        .collect();
    for t in threads {
        t.join().unwrap();
    }
    assert_eq!(r.count(), 8000);
}

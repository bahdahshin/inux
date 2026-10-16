// SPDX-License-Identifier: GPL-2.0-or-later
//! Host tests for the wait queue: placement at head and tail, the wake walk,
//! exclusive counting and removal. Linux has no KUnit file for this list.

use rinux_lib::wait::WaitQueue;

#[test]
fn non_exclusive_waiters_go_to_the_head_and_all_wake() {
    let mut q: WaitQueue<4> = WaitQueue::new();
    q.add(0);
    q.add(1);
    let mut woken = Vec::new();
    let left = q.wake_up_common(1, |id| {
        woken.push(id);
        1
    });
    // Head insertion: 1 was added last, so it is first.
    assert_eq!(woken, vec![1, 0]);
    assert_eq!(left, 1);
}

#[test]
fn exclusive_waiters_are_woken_up_to_the_count() {
    let mut q: WaitQueue<4> = WaitQueue::new();
    q.add(0);
    q.add_exclusive(1);
    q.add_exclusive(2);
    q.add_exclusive(3);
    let mut woken = Vec::new();
    let left = q.wake_up_common(2, |id| {
        woken.push(id);
        1
    });
    // Non-exclusive 0 always wakes; exclusive 1 and 2 use the two slots.
    assert_eq!(woken, vec![0, 1, 2]);
    assert_eq!(left, 0);
}

#[test]
fn a_negative_wake_stops_the_walk() {
    let mut q: WaitQueue<3> = WaitQueue::new();
    q.add(0);
    q.add(1);
    q.add(2);
    let mut seen = 0;
    q.wake_up_common(0, |_| {
        seen += 1;
        if seen == 2 { -1 } else { 1 }
    });
    assert_eq!(seen, 2);
}

#[test]
fn removal_unlinks_and_reports_absence() {
    let mut q: WaitQueue<3> = WaitQueue::new();
    q.add(0);
    q.add(1);
    assert!(q.remove(0));
    assert!(!q.remove(0));
    let mut woken = Vec::new();
    q.wake_up_common(0, |id| {
        woken.push(id);
        1
    });
    assert_eq!(woken, vec![1]);
}

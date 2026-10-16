// SPDX-License-Identifier: GPL-2.0-or-later
//! Host tests for the RT priority array: highest priority first, FIFO within a
//! priority, removal from the middle, and the bitmap with its delimiter bit.
//! Linux has no KUnit file for this structure.

use rinux_lib::sched_rt_prio::{MAX_RT_PRIO, RtPrioArray};

#[test]
fn picks_the_highest_priority_first() {
    let mut a: RtPrioArray<8> = RtPrioArray::new();
    assert_eq!(a.pick_first(), None);
    a.enqueue(50, 0);
    a.enqueue(10, 1);
    a.enqueue(99, 2);
    assert_eq!(a.pick_first(), Some((10, 1)));
    assert!(a.dequeue(1));
    assert_eq!(a.pick_first(), Some((50, 0)));
}

#[test]
fn equal_priorities_are_fifo() {
    let mut a: RtPrioArray<4> = RtPrioArray::new();
    a.enqueue(7, 0);
    a.enqueue(7, 1);
    a.enqueue(7, 2);
    assert_eq!(a.pick_first(), Some((7, 0)));
    a.dequeue(0);
    assert_eq!(a.pick_first(), Some((7, 1)));
}

#[test]
fn removing_from_the_middle_keeps_the_rest_linked() {
    let mut a: RtPrioArray<4> = RtPrioArray::new();
    for id in 0..3 {
        a.enqueue(20, id);
    }
    assert!(a.dequeue(1));
    assert_eq!(a.pick_first(), Some((20, 0)));
    a.dequeue(0);
    assert_eq!(a.pick_first(), Some((20, 2)));
    a.dequeue(2);
    assert_eq!(a.pick_first(), None);
    assert!(!a.dequeue(2));
}

#[test]
fn priority_bitmap_clears_with_its_queue() {
    let mut a: RtPrioArray<2> = RtPrioArray::new();
    a.enqueue(MAX_RT_PRIO - 1, 0);
    a.enqueue(0, 1);
    a.dequeue(1);
    assert_eq!(a.pick_first(), Some((MAX_RT_PRIO - 1, 0)));
    a.dequeue(0);
    assert_eq!(a.pick_first(), None);
}

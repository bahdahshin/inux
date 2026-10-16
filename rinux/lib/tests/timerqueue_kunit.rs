// SPDX-License-Identifier: GPL-2.0-or-later
//! Tests for the timer queue.

use rinux_lib::timerqueue::{TimerqueueHead, TimerqueueNode};

/// Collect the queue's nodes in expiry order starting from the cached next.
unsafe fn collect(head: &TimerqueueHead) -> Vec<u64> {
    let mut out = Vec::new();
    let mut cur = head.getnext();
    while !cur.is_null() {
        unsafe {
            out.push((*cur).expires);
            cur = TimerqueueHead::iterate_next(cur);
        }
    }
    out
}

#[test]
fn ordered_and_cached_earliest() {
    let order = [50u64, 10, 30, 20, 40];
    let mut nodes: Vec<Box<TimerqueueNode>> = order
        .iter()
        .map(|&e| Box::new(TimerqueueNode::new(e)))
        .collect();
    let mut head = TimerqueueHead::new();

    let mut firsts = Vec::new();
    for n in nodes.iter_mut() {
        let p: *mut TimerqueueNode = &mut **n;
        unsafe { firsts.push(head.add(p)) };
    }
    // add returns true only when the new node is the earliest so far:
    // 50 (empty) -> true, 10 (new min) -> true, then 30/20/40 -> false.
    assert_eq!(firsts, vec![true, true, false, false, false]);

    unsafe {
        assert_eq!((*head.getnext()).expires, 10);
        assert_eq!(collect(&head), vec![10, 20, 30, 40, 50]);
    }
}

#[test]
fn del_advances_and_preserves() {
    let mut nodes: Vec<Box<TimerqueueNode>> = [10u64, 20, 30, 40]
        .iter()
        .map(|&e| Box::new(TimerqueueNode::new(e)))
        .collect();
    let ptrs: Vec<*mut TimerqueueNode> = nodes.iter_mut().map(|n| &mut **n as *mut _).collect();
    let mut head = TimerqueueHead::new();
    unsafe {
        for &p in &ptrs {
            head.add(p);
        }
        // Deleting a middle node leaves the cached earliest alone.
        assert!(head.del(ptrs[1])); // remove 20
        assert_eq!((*head.getnext()).expires, 10);
        assert_eq!(collect(&head), vec![10, 30, 40]);

        // Deleting the earliest advances the cache.
        assert!(head.del(ptrs[0])); // remove 10
        assert_eq!((*head.getnext()).expires, 30);

        assert!(head.del(ptrs[2])); // remove 30
        assert_eq!((*head.getnext()).expires, 40);

        // Emptying the queue clears the cache and returns false.
        assert!(!head.del(ptrs[3]));
        assert!(head.getnext().is_null());
    }
}

#[test]
fn matches_a_sorted_reference() {
    // Insert a shuffled set of expiries; the queue must iterate them sorted,
    // with the cached next always the minimum.
    let mut seed: u32 = 0x5151_2719;
    let mut rng = || {
        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        seed
    };
    let mut expiries: Vec<u64> = (0..64).map(|_| (rng() % 1000) as u64).collect();
    let mut nodes: Vec<Box<TimerqueueNode>> = expiries
        .iter()
        .map(|&e| Box::new(TimerqueueNode::new(e)))
        .collect();
    let mut head = TimerqueueHead::new();
    unsafe {
        for n in nodes.iter_mut() {
            head.add(&mut **n);
        }
        expiries.sort_unstable();
        assert_eq!(collect(&head), expiries);
    }
}

// SPDX-License-Identifier: GPL-2.0-or-later
//! Host tests for the lwq queue order. Linux has no KUnit file for it; these
//! check first-in first-out order across the producer and consumer lists.

use rinux_lib::lwq::Lwq;

fn drain(q: &mut Lwq<'_>) -> Vec<usize> {
    let mut out = Vec::new();
    while let Some(id) = q.dequeue() {
        out.push(id);
    }
    out
}

#[test]
fn dequeue_is_first_in_first_out() {
    let mut links = [None; 5];
    let mut q = Lwq::new(&mut links);
    for id in 0..5 {
        q.enqueue(id);
    }
    assert_eq!(drain(&mut q), vec![0, 1, 2, 3, 4]);
    assert!(q.is_empty());
}

#[test]
fn enqueue_reports_the_empty_to_nonempty_transition() {
    let mut links = [None; 3];
    let mut q = Lwq::new(&mut links);
    assert!(q.enqueue(0));
    assert!(!q.enqueue(1));
    q.dequeue();
    q.dequeue();
    assert!(q.enqueue(2));
}

#[test]
fn items_added_after_a_partial_drain_keep_their_order() {
    let mut links = [None; 4];
    let mut q = Lwq::new(&mut links);
    q.enqueue(0);
    q.enqueue(1);
    assert_eq!(q.dequeue(), Some(0));
    q.enqueue(2);
    assert_eq!(drain(&mut q), vec![1, 2]);
}

#[test]
fn dequeue_all_returns_oldest_first_across_both_lists() {
    let mut links = [None; 4];
    let mut q = Lwq::new(&mut links);
    q.enqueue(0);
    q.enqueue(1);
    assert_eq!(q.dequeue(), Some(0)); // now 1 is in `ready`
    q.enqueue(2); // and 2 is in `new`
    let head = q.dequeue_all().unwrap();
    let mut seen = vec![head];
    while let Some(next) = q.next_of(*seen.last().unwrap()) {
        seen.push(next);
    }
    assert_eq!(seen, vec![1, 2]);
    assert!(q.is_empty());
    assert_eq!(q.dequeue_all(), None);
}

#[test]
fn empty_queue_dequeues_nothing() {
    let mut links = [None; 2];
    let mut q = Lwq::new(&mut links);
    assert_eq!(q.dequeue(), None);
}

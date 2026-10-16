// SPDX-License-Identifier: GPL-2.0-or-later
//! Host tests for the lwq queue. Linux has no KUnit file for it; these check
//! first-in first-out order, `dequeue_all`, and concurrent producers.

use rinux_lib::lwq::{Lwq, NONE, blank_links};

fn drain(q: &Lwq<'_>) -> Vec<usize> {
    let mut out = Vec::new();
    while let Some(id) = q.dequeue() {
        out.push(id);
    }
    out
}

#[test]
fn dequeue_is_first_in_first_out() {
    let links = blank_links::<5>();
    let q = Lwq::new(&links);
    for id in 0..5 {
        q.enqueue(id);
    }
    assert_eq!(drain(&q), vec![0, 1, 2, 3, 4]);
    assert!(q.is_empty());
}

#[test]
fn enqueue_reports_when_the_stack_was_empty() {
    let links = blank_links::<3>();
    let q = Lwq::new(&links);
    assert!(q.enqueue(0));
    assert!(!q.enqueue(1));
}

#[test]
fn items_added_after_a_partial_drain_keep_their_order() {
    let links = blank_links::<4>();
    let q = Lwq::new(&links);
    q.enqueue(0);
    q.enqueue(1);
    assert_eq!(q.dequeue(), Some(0));
    q.enqueue(2);
    assert_eq!(drain(&q), vec![1, 2]);
}

#[test]
fn dequeue_all_returns_oldest_first_across_both_lists() {
    let links = blank_links::<4>();
    let q = Lwq::new(&links);
    q.enqueue(0);
    q.enqueue(1);
    assert_eq!(q.dequeue(), Some(0)); // 1 is now in `ready`
    q.enqueue(2); // and 2 is in `new`
    let head = q.dequeue_all().unwrap();
    let mut seen = vec![head];
    loop {
        let n = q.next_of(*seen.last().unwrap());
        if n == NONE {
            break;
        }
        seen.push(n);
    }
    assert_eq!(seen, vec![1, 2]);
    assert!(q.is_empty());
    assert_eq!(q.dequeue_all(), None);
}

#[test]
fn empty_queue_dequeues_nothing() {
    let links = blank_links::<2>();
    let q = Lwq::new(&links);
    assert_eq!(q.dequeue(), None);
}

#[test]
fn concurrent_producers_and_a_consumer_see_every_item_once_in_order() {
    const PRODUCERS: usize = 4;
    const PER: usize = 250;
    let links = blank_links::<{ PRODUCERS * PER }>();
    let q = Lwq::new(&links);
    let mut got = Vec::new();

    std::thread::scope(|s| {
        for p in 0..PRODUCERS {
            let q = &q;
            s.spawn(move || {
                for i in 0..PER {
                    q.enqueue(p * PER + i);
                }
            });
        }
        // The consumer runs alongside the producers, then drains the rest.
        while got.len() < PRODUCERS * PER {
            match q.dequeue() {
                Some(id) => got.push(id),
                None => std::hint::spin_loop(),
            }
        }
    });

    // Every item once.
    let mut sorted = got.clone();
    sorted.sort_unstable();
    assert_eq!(sorted, (0..PRODUCERS * PER).collect::<Vec<_>>());
    // Each producer's items come out in the order it added them.
    for p in 0..PRODUCERS {
        let mine: Vec<usize> = got.iter().copied().filter(|&id| id / PER == p).collect();
        let mut expected = mine.clone();
        expected.sort_unstable();
        assert_eq!(mine, expected, "producer {p} out of order");
    }
}

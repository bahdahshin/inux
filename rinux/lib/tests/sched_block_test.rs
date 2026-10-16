// SPDX-License-Identifier: GPL-2.0
//! Host tests for the sleeping wait paths: `wait_event()`, `wake_up()` and
//! `wait_for_completion()` / `complete_wake()`, over a [`TaskBlocker`] double.
//! The double records sleeps and wakes; for a completion it also completes
//! the waiter's completion, which models the waker running while the task is
//! asleep.

use rinux_lib::completion::{Completion, complete_wake, wait_for_completion};
use rinux_lib::task_block::TaskBlocker;
use rinux_lib::wait::{WaitQueue, wait_event, wake_up};

#[derive(Default)]
struct Double {
    blocked: Vec<usize>,
    woken: Vec<usize>,
}

impl TaskBlocker for Double {
    fn block(&mut self, id: usize) {
        self.blocked.push(id);
    }
    fn wake(&mut self, id: usize) {
        self.woken.push(id);
    }
}

/// A blocker whose `block` completes the completion, as the waker would.
struct Completer<'a> {
    c: &'a Completion,
    blocked: Vec<usize>,
}

impl TaskBlocker for Completer<'_> {
    fn block(&mut self, id: usize) {
        self.blocked.push(id);
        self.c.complete();
    }
    fn wake(&mut self, _id: usize) {}
}

#[test]
fn wait_event_returns_without_sleeping_when_the_condition_holds() {
    let mut q: WaitQueue<4> = WaitQueue::new();
    let mut b = Double::default();
    wait_event(&mut q, &mut b, 0, || true);
    assert!(b.blocked.is_empty());
    assert!(q.is_empty());
}

#[test]
fn wait_event_sleeps_until_the_condition_holds_and_leaves_the_queue() {
    let mut q: WaitQueue<4> = WaitQueue::new();
    let mut b = Double::default();
    let mut checks = 0;
    // False on the first check, true after the task has been put to sleep.
    wait_event(&mut q, &mut b, 2, || {
        checks += 1;
        checks > 2
    });
    assert_eq!(b.blocked, vec![2]);
    assert!(q.is_empty(), "the task leaves the queue after sleeping");
}

#[test]
fn wait_for_completion_takes_a_ready_completion_without_sleeping() {
    let c = Completion::new();
    c.complete();
    let mut q: WaitQueue<4> = WaitQueue::new();
    let mut b = Completer {
        c: &c,
        blocked: vec![],
    };
    wait_for_completion(&c, &mut q, &mut b, 0);
    assert!(b.blocked.is_empty());
    assert!(!c.is_done(), "the one completion was taken");
}

#[test]
fn wait_for_completion_sleeps_and_is_passed_by_the_completer() {
    let c = Completion::new();
    let mut q: WaitQueue<4> = WaitQueue::new();
    let mut b = Completer {
        c: &c,
        blocked: vec![],
    };
    wait_for_completion(&c, &mut q, &mut b, 1);
    assert_eq!(b.blocked, vec![1]);
    assert!(!c.is_done());
}

#[test]
fn complete_wake_wakes_one_exclusive_waiter() {
    let c = Completion::new();
    let mut q: WaitQueue<4> = WaitQueue::new();
    q.add_exclusive(0);
    q.add_exclusive(1);
    let mut b = Double::default();
    complete_wake(&c, &mut q, &mut b);
    assert_eq!(b.woken, vec![0]);
    assert!(c.is_done());
}

#[test]
fn wake_up_with_no_limit_wakes_every_exclusive_waiter() {
    let mut q: WaitQueue<4> = WaitQueue::new();
    q.add_exclusive(0);
    q.add_exclusive(1);
    let mut b = Double::default();
    let left = wake_up(&mut q, &mut b, 0);
    assert_eq!(b.woken, vec![0, 1]);
    // C keeps decrementing with no limit, so the leftover count goes negative
    // (-1 for each exclusive waiter woken).
    assert_eq!(left, -2);
}

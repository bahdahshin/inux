// SPDX-License-Identifier: GPL-2.0
//! A first-in, first-out queue that producers can add to without a lock.
//!
//! Ported from `lib/lwq.c` and `include/linux/lwq.h`. C's lwq has two lists:
//! `new`, a lock-free stack that producers push onto with `cmpxchg`, and
//! `ready`, which consumers take from in arrival order. When `ready` runs out,
//! a consumer takes all of `new` under a spinlock and reverses it into `ready`.
//!
//! Nodes are ids: indices into a caller's `next` slice of atomics, with
//! [`NONE`] as the null link. Every method takes `&self`, so one queue can be
//! shared by producers and consumers on several threads, as in C. The
//! `ready` marker that C sets while it moves `new` across (`(void *)1`) is the
//! [`TRANSFER`] value here, so `is_empty()` never reads a transient empty.

use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

/// The null link: no node.
pub const NONE: usize = usize::MAX;
/// `ready` while a consumer is moving `new` into it.
const TRANSFER: usize = usize::MAX - 1;

/// An lwq over the node ids `0..next.len()`.
#[derive(Debug)]
pub struct Lwq<'a> {
    next: &'a [AtomicUsize],
    /// `new`: the producer stack, newest first.
    new: AtomicUsize,
    /// `ready`: the consumer list, oldest first.
    ready: AtomicUsize,
    /// The spinlock that guards `ready` and the transfer from `new`.
    lock: AtomicBool,
}

impl<'a> Lwq<'a> {
    /// `lwq_init()`: an empty queue over the link storage `next`, which must
    /// start as all [`NONE`] (see [`blank_links`]).
    pub const fn new(next: &'a [AtomicUsize]) -> Self {
        Self {
            next,
            new: AtomicUsize::new(NONE),
            ready: AtomicUsize::new(NONE),
            lock: AtomicBool::new(false),
        }
    }

    /// `lwq_empty()`: true when neither list has an entry.
    pub fn is_empty(&self) -> bool {
        self.new.load(Ordering::Acquire) == NONE && self.ready.load(Ordering::Acquire) == NONE
    }

    /// `lwq_enqueue()`: add node `id` at the end. Lock-free. Returns true
    /// when `new` was empty, which is when C wakes a waiter.
    pub fn enqueue(&self, id: usize) -> bool {
        let mut head = self.new.load(Ordering::Relaxed);
        loop {
            self.next[id].store(head, Ordering::Relaxed);
            match self
                .new
                .compare_exchange_weak(head, id, Ordering::Release, Ordering::Relaxed)
            {
                Ok(_) => return head == NONE,
                Err(cur) => head = cur,
            }
        }
    }

    /// `__lwq_dequeue()`: remove the oldest node, if any.
    pub fn dequeue(&self) -> Option<usize> {
        if self.is_empty() {
            return None;
        }
        self.lock();
        let mut this = self.ready.load(Ordering::Relaxed);
        if this == NONE && self.new.load(Ordering::Acquire) != NONE {
            // Keep the queue visibly non-empty while the stack moves over.
            self.ready.store(TRANSFER, Ordering::Release);
            let taken = self.new.swap(NONE, Ordering::AcqRel);
            this = self.reverse(taken);
            if this == NONE {
                self.ready.store(NONE, Ordering::Release);
            }
        }
        if this != NONE {
            let after = self.next[this].load(Ordering::Relaxed);
            self.ready.store(after, Ordering::Release);
        }
        self.unlock();
        if this == NONE { None } else { Some(this) }
    }

    /// `lwq_dequeue_all()`: take every node, oldest first, as one list. Walk
    /// it with [`Lwq::next_of`] until [`NONE`].
    pub fn dequeue_all(&self) -> Option<usize> {
        if self.is_empty() {
            return None;
        }
        self.lock();
        let r = self.ready.swap(NONE, Ordering::AcqRel);
        let t = self.new.swap(NONE, Ordering::AcqRel);
        self.unlock();
        let tail = self.reverse(t);
        // Append `tail` after `r`; both are detached from the queue now.
        if r == NONE {
            return if tail == NONE { None } else { Some(tail) };
        }
        let mut last = r;
        loop {
            let n = self.next[last].load(Ordering::Relaxed);
            if n == NONE {
                break;
            }
            last = n;
        }
        self.next[last].store(tail, Ordering::Relaxed);
        Some(r)
    }

    /// The node after `id` in a list returned by [`Lwq::dequeue_all`], or
    /// [`NONE`] at its end.
    pub fn next_of(&self, id: usize) -> usize {
        self.next[id].load(Ordering::Relaxed)
    }

    fn lock(&self) {
        while self
            .lock
            .compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            core::hint::spin_loop();
        }
    }

    fn unlock(&self) {
        self.lock.store(false, Ordering::Release);
    }

    /// `llist_reverse_order()`: reverse a list of ids and return its new head.
    fn reverse(&self, mut head: usize) -> usize {
        let mut prev = NONE;
        while head != NONE {
            let next = self.next[head].load(Ordering::Relaxed);
            self.next[head].store(prev, Ordering::Relaxed);
            prev = head;
            head = next;
        }
        prev
    }
}

/// `next` storage for `N` nodes, all unlinked.
pub const fn blank_links<const N: usize>() -> [AtomicUsize; N] {
    [const { AtomicUsize::new(NONE) }; N]
}

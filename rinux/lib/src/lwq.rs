// SPDX-License-Identifier: GPL-2.0
//! A first-in, first-out queue that adds at one end cheaply.
//!
//! Ported from `lib/lwq.c` and `include/linux/lwq.h`. C's lwq has two lists:
//! `new`, a lock-free stack that producers push onto, and `ready`, which
//! consumers take from in arrival order. When `ready` runs out, the consumer
//! reverses `new` into it, under a spinlock.
//!
//! This port keeps that state machine and its ordering rules over node ids
//! (indices into a caller's `next` slice), not raw pointers. It is the
//! sequential core: it takes `&mut self`, so it is not safe to share between
//! CPUs as C's is. The lock-free producer side needs the kernel's shared-access
//! wrapper, which is not written yet.

/// An lwq over `next.len()` node ids. Node `i` is in the queue when it has
/// been enqueued and not yet dequeued; `next[i]` links it to the following id.
#[derive(Debug)]
pub struct Lwq<'a> {
    next: &'a mut [Option<usize>],
    /// `new`: the producer stack, newest first.
    new: Option<usize>,
    /// `ready`: the consumer list, oldest first.
    ready: Option<usize>,
}

impl<'a> Lwq<'a> {
    /// `lwq_init()`: an empty queue over the link storage `next`.
    pub fn new(next: &'a mut [Option<usize>]) -> Self {
        Self {
            next,
            new: None,
            ready: None,
        }
    }

    /// `lwq_empty()`: true when neither list has an entry.
    pub fn is_empty(&self) -> bool {
        self.new.is_none() && self.ready.is_none()
    }

    /// `lwq_enqueue()`: add node `id` at the end. Returns true when the queue
    /// was empty, which is when C wakes a waiter.
    pub fn enqueue(&mut self, id: usize) -> bool {
        let was_empty = self.is_empty();
        self.next[id] = self.new;
        self.new = Some(id);
        was_empty
    }

    /// `__lwq_dequeue()`: remove the oldest node, if any.
    pub fn dequeue(&mut self) -> Option<usize> {
        if self.is_empty() {
            return None;
        }
        if self.ready.is_none() && self.new.is_some() {
            // Move the producer stack over, reversed, to get arrival order.
            let taken = self.new.take();
            self.ready = self.reverse(taken);
        }
        let this = self.ready?;
        self.ready = self.next[this];
        Some(this)
    }

    /// `lwq_dequeue_all()`: take every node, oldest first, as one list.
    /// Walk the list with [`Lwq::next_of`].
    pub fn dequeue_all(&mut self) -> Option<usize> {
        if self.is_empty() {
            return None;
        }
        let r = self.ready.take();
        let t = self.new.take();
        let tail = self.reverse(t);
        // Append `tail` after `r`.
        match r {
            None => tail,
            Some(mut last) => {
                while let Some(n) = self.next[last] {
                    last = n;
                }
                self.next[last] = tail;
                r
            }
        }
    }

    /// The node after `id` in a list returned by [`Lwq::dequeue_all`].
    pub fn next_of(&self, id: usize) -> Option<usize> {
        self.next[id]
    }

    /// `llist_reverse_order()`: reverse a list of ids in place.
    fn reverse(&mut self, mut head: Option<usize>) -> Option<usize> {
        let mut prev = None;
        while let Some(cur) = head {
            head = self.next[cur];
            self.next[cur] = prev;
            prev = Some(cur);
        }
        prev
    }
}

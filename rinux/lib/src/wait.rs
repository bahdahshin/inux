// SPDX-License-Identifier: GPL-2.0-only
//! Wait queues: the ordered list of waiters and the wake-up walk.
//!
//! Ported from `kernel/sched/wait.c`: `add_wait_queue()` (head),
//! `add_wait_queue_exclusive()` (tail), `remove_wait_queue()` and
//! `__wake_up_common()`. Waiters are entry ids into a caller's storage, in place
//! of `wait_queue_entry_t`, and the wake function is a closure whose return value
//! has C's meaning: negative stops the walk, positive means the waiter was woken,
//! zero means it was skipped. The spinlock that serialises the list in C is the
//! caller's, as the kernel's lock belongs to the kernel.
//!
//! [`wait_event`] and [`wake_up`] are the sleeping paths; they take a
//! [`crate::task_block::TaskBlocker`] for the scheduler's sleep and wake.

use crate::task_block::TaskBlocker;

/// `WQ_FLAG_EXCLUSIVE`: at most `nr_exclusive` of these are woken.
pub const WQ_FLAG_EXCLUSIVE: u32 = 0x01;

/// One waiter: its id and flags, with links into the queue.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Entry {
    flags: u32,
    prev: Option<usize>,
    next: Option<usize>,
    queued: bool,
}

/// `struct wait_queue_head`, over `N` waiter ids.
#[derive(Debug)]
pub struct WaitQueue<const N: usize> {
    head: Option<usize>,
    tail: Option<usize>,
    entries: [Entry; N],
}

impl<const N: usize> Default for WaitQueue<N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize> WaitQueue<N> {
    /// An empty wait queue.
    pub const fn new() -> Self {
        Self {
            head: None,
            tail: None,
            entries: [Entry {
                flags: 0,
                prev: None,
                next: None,
                queued: false,
            }; N],
        }
    }

    /// `add_wait_queue()`: a non-exclusive waiter goes at the head.
    /// `list_empty()` on the queue head: no waiters are queued.
    pub fn is_empty(&self) -> bool {
        self.head.is_none()
    }

    pub fn add(&mut self, id: usize) {
        self.entries[id].flags &= !WQ_FLAG_EXCLUSIVE;
        self.link_head(id);
    }

    /// `add_wait_queue_exclusive()`: an exclusive waiter goes at the tail.
    pub fn add_exclusive(&mut self, id: usize) {
        self.entries[id].flags |= WQ_FLAG_EXCLUSIVE;
        self.link_tail(id);
    }

    fn link_head(&mut self, id: usize) {
        assert!(!self.entries[id].queued, "waiter {id} already queued");
        self.entries[id].queued = true;
        self.entries[id].prev = None;
        self.entries[id].next = self.head;
        match self.head {
            Some(h) => self.entries[h].prev = Some(id),
            None => self.tail = Some(id),
        }
        self.head = Some(id);
    }

    fn link_tail(&mut self, id: usize) {
        assert!(!self.entries[id].queued, "waiter {id} already queued");
        self.entries[id].queued = true;
        self.entries[id].next = None;
        self.entries[id].prev = self.tail;
        match self.tail {
            Some(t) => self.entries[t].next = Some(id),
            None => self.head = Some(id),
        }
        self.tail = Some(id);
    }

    /// `remove_wait_queue()`: unlink a waiter. Returns false if it was not queued.
    pub fn remove(&mut self, id: usize) -> bool {
        if !self.entries[id].queued {
            return false;
        }
        let Entry { prev, next, .. } = self.entries[id];
        match prev {
            Some(p) => self.entries[p].next = next,
            None => self.head = next,
        }
        match next {
            Some(n) => self.entries[n].prev = prev,
            None => self.tail = prev,
        }
        self.entries[id] = Entry {
            flags: self.entries[id].flags,
            prev: None,
            next: None,
            queued: false,
        };
        true
    }

    /// `__wake_up_common()`: walk the queue in order, calling `wake(id)` on each
    /// waiter. A negative result stops the walk; an exclusive waiter that was
    /// woken counts against `nr_exclusive`, and the walk stops when that count
    /// runs out. Returns what is left of `nr_exclusive`.
    pub fn wake_up_common(
        &mut self,
        mut nr_exclusive: i32,
        mut wake: impl FnMut(usize) -> i32,
    ) -> i32 {
        let mut cur = self.head;
        while let Some(id) = cur {
            let flags = self.entries[id].flags;
            let next = self.entries[id].next;
            let ret = wake(id);
            if ret < 0 {
                break;
            }
            if ret > 0 && flags & WQ_FLAG_EXCLUSIVE != 0 {
                nr_exclusive -= 1;
                if nr_exclusive == 0 {
                    break;
                }
            }
            cur = next;
        }
        nr_exclusive
    }
}

/// `wait_event()`: sleep task `id` until `cond` holds.
///
/// Follows the C order: check the condition, join the queue, check again
/// (so a wake between the two is not lost), block, then leave the queue.
/// `cond` is called with the waiter already queued, as in `prepare_to_wait()`.
pub fn wait_event<const N: usize>(
    q: &mut WaitQueue<N>,
    blocker: &mut impl TaskBlocker,
    id: usize,
    mut cond: impl FnMut() -> bool,
) {
    loop {
        if cond() {
            return;
        }
        q.add(id);
        if cond() {
            q.remove(id);
            return;
        }
        blocker.block(id);
        q.remove(id);
    }
}

/// `__wake_up()`: wake up to `nr_exclusive` exclusive waiters, and every
/// non-exclusive one before them in the walk. Returns what is left of
/// `nr_exclusive`, as [`WaitQueue::wake_up_common`] does.
pub fn wake_up<const N: usize>(
    q: &mut WaitQueue<N>,
    blocker: &mut impl TaskBlocker,
    nr_exclusive: i32,
) -> i32 {
    q.wake_up_common(nr_exclusive, |id| {
        blocker.wake(id);
        1
    })
}

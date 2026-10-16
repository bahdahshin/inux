// SPDX-License-Identifier: GPL-2.0-only
//! The priority array of the real-time scheduling class: one FIFO queue per
//! priority, and a bitmap that says which queues are non-empty.
//!
//! Ported from `struct rt_prio_array` in `kernel/sched/sched.h` and the way
//! `kernel/sched/rt.c` enqueues, dequeues and picks: the highest priority is the
//! lowest index, and a set bit in the bitmap marks a non-empty queue. Like C,
//! the bitmap has one extra bit (the delimiter) set at `MAX_RT_PRIO`, so the
//! search always stops. Queue links are entry ids into a caller's storage, in
//! place of `list_head`, so nothing is allocated.

/// `MAX_RT_PRIO`: the number of real-time priorities.
pub const MAX_RT_PRIO: usize = 100;

/// `struct rt_prio_array` over `N` entity ids.
#[derive(Debug)]
pub struct RtPrioArray<const N: usize> {
    /// Bit `p` set when queue `p` is non-empty; bit `MAX_RT_PRIO` is the
    /// delimiter and always set.
    bitmap: [u64; 2],
    head: [Option<usize>; MAX_RT_PRIO],
    tail: [Option<usize>; MAX_RT_PRIO],
    next: [Option<usize>; N],
    prev: [Option<usize>; N],
    /// The priority each entity is queued at, while it is queued.
    prio_of: [Option<usize>; N],
}

impl<const N: usize> Default for RtPrioArray<N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize> RtPrioArray<N> {
    /// An empty array, with the delimiter bit set.
    pub fn new() -> Self {
        let mut bitmap = [0u64; 2];
        bitmap[MAX_RT_PRIO / 64] |= 1 << (MAX_RT_PRIO % 64);
        Self {
            bitmap,
            head: [None; MAX_RT_PRIO],
            tail: [None; MAX_RT_PRIO],
            next: [None; N],
            prev: [None; N],
            prio_of: [None; N],
        }
    }

    fn set_bit(&mut self, p: usize) {
        self.bitmap[p / 64] |= 1 << (p % 64);
    }

    fn clear_bit(&mut self, p: usize) {
        self.bitmap[p / 64] &= !(1 << (p % 64));
    }

    /// `enqueue_task_rt()` for one entity: append `id` to the queue at `prio`.
    /// Panics if `prio` is out of range or the entity is already queued.
    pub fn enqueue(&mut self, prio: usize, id: usize) {
        assert!(prio < MAX_RT_PRIO, "RT priority {prio} out of range");
        assert!(self.prio_of[id].is_none(), "entity {id} already queued");
        self.prio_of[id] = Some(prio);
        self.prev[id] = self.tail[prio];
        self.next[id] = None;
        match self.tail[prio] {
            Some(t) => self.next[t] = Some(id),
            None => self.head[prio] = Some(id),
        }
        self.tail[prio] = Some(id);
        self.set_bit(prio);
    }

    /// `dequeue_task_rt()` for one entity: unlink `id`, clearing the priority's
    /// bit when its queue empties. Returns false if `id` was not queued.
    pub fn dequeue(&mut self, id: usize) -> bool {
        let Some(prio) = self.prio_of[id].take() else {
            return false;
        };
        match self.prev[id] {
            Some(p) => self.next[p] = self.next[id],
            None => self.head[prio] = self.next[id],
        }
        match self.next[id] {
            Some(n) => self.prev[n] = self.prev[id],
            None => self.tail[prio] = self.prev[id],
        }
        self.prev[id] = None;
        self.next[id] = None;
        if self.head[prio].is_none() {
            self.clear_bit(prio);
        }
        true
    }

    /// `sched_find_first_bit()`: the highest non-empty priority, and the entity
    /// at the front of its queue. `None` when nothing is queued.
    pub fn pick_first(&self) -> Option<(usize, usize)> {
        let p = self.bitmap.iter().enumerate().find_map(|(word, &bits)| {
            (bits != 0).then(|| word * 64 + bits.trailing_zeros() as usize)
        })?;
        if p >= MAX_RT_PRIO {
            // Only the delimiter is set.
            return None;
        }
        self.head[p].map(|id| (p, id))
    }
}

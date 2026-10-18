// SPDX-License-Identifier: GPL-2.0
//! The high-resolution timer queue: expiry order, cancel, restart, and the
//! overrun count for a periodic timer.
//!
//! Ported from `kernel/time/hrtimer.c`: `hrtimer_forward()`, and the queue
//! operations of `enqueue_hrtimer()`, `remove_hrtimer()`,
//! `__hrtimer_start_range_ns()` and `__run_hrtimer()`, over one clock base.
//! Linux keeps the timers in a red-black tree and runs them with the base
//! lock dropped around each callback. Here the timers are a sorted array of
//! fixed capacity, so the module needs no allocator, and a timer is named by
//! a caller-chosen id. The per-CPU bases, clock-event reprogramming, softirq
//! expiry and the SMP migration paths are not ported.

/// The queue is full: `start()` could not insert the timer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QueueFull;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Entry {
    expires: i64,
    seq: u64,
    id: usize,
}

/// A timer queue of at most `N` timers, in (expiry, insertion) order.
#[derive(Debug)]
pub struct TimerQueue<const N: usize> {
    entries: [Entry; N],
    len: usize,
    seq: u64,
}

impl<const N: usize> Default for TimerQueue<N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize> TimerQueue<N> {
    /// An empty queue.
    pub const fn new() -> Self {
        Self {
            entries: [Entry {
                expires: 0,
                seq: 0,
                id: 0,
            }; N],
            len: 0,
            seq: 0,
        }
    }

    /// The queued timers, earliest first.
    fn as_slice(&self) -> &[Entry] {
        &self.entries[..self.len]
    }

    /// `remove_hrtimer()`: take a timer off the queue. Returns whether it was
    /// queued.
    pub fn cancel(&mut self, id: usize) -> bool {
        let Some(pos) = self.position(id) else {
            return false;
        };
        self.remove_at(pos);
        true
    }

    /// `__hrtimer_start_range_ns()`: queue a timer to expire at `expires`
    /// nanoseconds. A timer that is already queued is moved, as in C. Returns
    /// whether it is now the earliest timer.
    pub fn start(&mut self, id: usize, expires: i64) -> Result<bool, QueueFull> {
        self.cancel(id);
        if self.len == N {
            return Err(QueueFull);
        }
        self.seq += 1;
        let entry = Entry {
            expires,
            seq: self.seq,
            id,
        };
        // Insert after every timer that expires no later, so ties keep their
        // insertion order.
        let pos = self.as_slice().partition_point(|e| e.expires <= expires);
        self.entries.copy_within(pos..self.len, pos + 1);
        self.entries[pos] = entry;
        self.len += 1;
        Ok(pos == 0)
    }

    /// `hrtimer_get_next_event()`: the earliest expiry, if any timer is queued.
    pub fn next_expiry(&self) -> Option<i64> {
        self.as_slice().first().map(|e| e.expires)
    }

    /// `__run_hrtimer()`'s queue step: remove and return the earliest timer
    /// whose expiry is at or before `now`. The caller runs its callback.
    pub fn pop_expired(&mut self, now: i64) -> Option<usize> {
        let first = *self.as_slice().first()?;
        if first.expires > now {
            return None;
        }
        self.remove_at(0);
        Some(first.id)
    }

    fn position(&self, id: usize) -> Option<usize> {
        self.as_slice().iter().position(|e| e.id == id)
    }

    fn remove_at(&mut self, pos: usize) {
        self.entries.copy_within(pos + 1..self.len, pos);
        self.len -= 1;
    }
}

/// `hrtimer_forward()`: move a periodic timer's expiry past `now` by whole
/// intervals, and return how many expiries were missed (the overrun count).
///
/// `expires` is the timer's current expiry, updated in place. `interval` is
/// raised to `resolution` when smaller. A timer that is still queued cannot
/// be moved, so `queued` makes the result 0, as the C `WARN_ON` path does.
pub fn forward(expires: &mut i64, now: i64, interval: i64, resolution: i64, queued: bool) -> u64 {
    let delta = now - *expires;
    if delta < 0 {
        return 0;
    }
    if queued {
        return 0;
    }
    let interval = interval.max(resolution);

    let mut orun = 1u64;
    if delta >= interval {
        let incr = interval;
        orun = (delta / incr) as u64;
        *expires += incr * orun as i64;
        if *expires > now {
            return orun;
        }
        // The correction for the exact count: one more expiry is due.
        orun += 1;
    }
    *expires += interval;
    orun
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forward_before_expiry_is_a_no_op() {
        let mut e = 100;
        assert_eq!(forward(&mut e, 50, 10, 1, false), 0);
        assert_eq!(e, 100);
    }

    #[test]
    fn forward_queued_timer_is_a_no_op() {
        let mut e = 0;
        assert_eq!(forward(&mut e, 25, 10, 1, true), 0);
        assert_eq!(e, 0);
    }

    #[test]
    fn forward_past_several_intervals_counts_the_overruns() {
        // Expired at 0, now 25, interval 10: expiries at 0, 10 and 20 were
        // missed, and the next one is at 30.
        let mut e = 0;
        assert_eq!(forward(&mut e, 25, 10, 1, false), 3);
        assert_eq!(e, 30);
    }

    #[test]
    fn forward_exactly_on_an_expiry_advances_one_interval() {
        let mut e = 0;
        assert_eq!(forward(&mut e, 10, 10, 1, false), 2);
        assert_eq!(e, 20);
    }

    #[test]
    fn interval_below_resolution_is_raised() {
        let mut e = 0;
        assert_eq!(forward(&mut e, 0, 1, 5, false), 1);
        assert_eq!(e, 5);
    }

    #[test]
    fn queue_runs_timers_in_expiry_order() {
        let mut q: TimerQueue<4> = TimerQueue::new();
        assert_eq!(q.start(1, 300), Ok(true));
        assert_eq!(q.start(2, 100), Ok(true));
        assert_eq!(q.start(3, 200), Ok(false));
        assert_eq!(q.next_expiry(), Some(100));
        assert_eq!(q.pop_expired(250), Some(2));
        assert_eq!(q.pop_expired(250), Some(3));
        assert_eq!(q.pop_expired(250), None);
        assert_eq!(q.pop_expired(300), Some(1));
        assert_eq!(q.next_expiry(), None);
    }

    #[test]
    fn equal_expiries_run_in_insertion_order() {
        let mut q: TimerQueue<4> = TimerQueue::new();
        q.start(7, 50).unwrap();
        q.start(8, 50).unwrap();
        assert_eq!(q.pop_expired(50), Some(7));
        assert_eq!(q.pop_expired(50), Some(8));
    }

    #[test]
    fn restarting_a_queued_timer_moves_it() {
        let mut q: TimerQueue<4> = TimerQueue::new();
        q.start(1, 100).unwrap();
        q.start(2, 200).unwrap();
        q.start(1, 300).unwrap();
        assert_eq!(q.next_expiry(), Some(200));
        assert_eq!(q.as_slice().len(), 2);
    }

    #[test]
    fn cancel_removes_only_that_timer() {
        let mut q: TimerQueue<4> = TimerQueue::new();
        q.start(1, 100).unwrap();
        q.start(2, 200).unwrap();
        assert!(q.cancel(1));
        assert!(!q.cancel(1));
        assert_eq!(q.pop_expired(1000), Some(2));
    }

    #[test]
    fn a_full_queue_refuses_a_new_timer() {
        let mut q: TimerQueue<2> = TimerQueue::new();
        q.start(1, 1).unwrap();
        q.start(2, 2).unwrap();
        assert_eq!(q.start(3, 3), Err(QueueFull));
        // Restarting a queued timer does not need a free slot.
        assert_eq!(q.start(1, 5), Ok(false));
    }
}

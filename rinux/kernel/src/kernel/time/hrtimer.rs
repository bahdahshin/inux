// SPDX-License-Identifier: GPL-2.0
//! High-resolution sleeps: the kernel side of `kernel/time/hrtimer.c`.
//!
//! The expiry queue is [`rinux_lib::hrtimer::TimerQueue`]. Each sleeper is
//! named by its task pointer, and the timer tick wakes every sleeper whose
//! deadline has passed, in deadline order. Deadlines are in nanoseconds of
//! [`local_clock`], so a sleep is not rounded up to a whole jiffy.

use rinux_lib::hrtimer::{QueueFull, TimerQueue};
use rinux_lib::task_block::TaskBlocker;

use crate::include::linux::sched::current;
use crate::kernel::locking::SpinLock;
use crate::kernel::sched::clock::local_clock;
use crate::kernel::sched::core::{RunqueueBlocker, sleep_until};
use crate::kernel::time::NSEC_PER_SEC;
use crate::kernel::time::jiffies::HZ;

/// Most sleepers the queue holds at once. A sleep that does not fit falls
/// back to the jiffy sleep, so it still returns.
const CAPACITY: usize = 64;

/// The armed high-resolution sleeps, earliest deadline first.
static QUEUE: SpinLock<TimerQueue<CAPACITY>> = SpinLock::new(TimerQueue::new());

/// Most expired sleepers one tick wakes. Any left wake on a later tick.
const WAKES_PER_TICK: usize = 32;

/// Sleeps the running task until `deadline` nanoseconds of [`local_clock`].
/// The task blocks between wakes, and the tick's [`run_expired`] wakes it
/// when the deadline passes. A deadline already past returns at once.
pub fn sleep_until_ns(deadline: u64) {
    let me = current() as usize;
    loop {
        if local_clock() >= deadline {
            break;
        }
        let armed = QUEUE.lock_irqsave().start(me, deadline as i64);
        if let Err(QueueFull) = armed {
            // No room for a timer: sleep to the next whole jiffy after the
            // deadline, which the tick's jiffy sleepers handle.
            let ns_per_tick = NSEC_PER_SEC / HZ;
            let jiffy_deadline = crate::kernel::time::jiffies::jiffies()
                + (deadline.saturating_sub(local_clock()) + ns_per_tick - 1) / ns_per_tick;
            sleep_until(jiffy_deadline);
            break;
        }
        // A wake that came before the block is kept as wakeup_pending, so
        // this returns at once and the loop re-checks the clock.
        RunqueueBlocker.block(me);
    }
    QUEUE.lock_irqsave().cancel(me);
}

/// Wakes the sleepers whose deadline is at or before `now`. Called from the
/// timer tick with interrupts off.
pub fn run_expired(now: u64) {
    for _ in 0..WAKES_PER_TICK {
        let due = QUEUE.lock_irqsave().pop_expired(now as i64);
        match due {
            // Wake outside the queue lock: wake() takes the runqueue lock.
            Some(id) => RunqueueBlocker.wake(id),
            None => break,
        }
    }
}

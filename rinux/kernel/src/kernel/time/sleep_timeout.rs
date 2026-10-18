// SPDX-License-Identifier: GPL-2.0
//! Sleeping for a number of jiffies: `schedule_timeout()` and `msleep()`.
//!
//! Ported from `kernel/time/sleep_timeout.c`. The C code arms a timer that
//! wakes the task and calls `schedule()`. Here the sleep is
//! [`crate::kernel::sched::core::sleep_until`], which parks the task in the
//! sleep table that the tick walks (step 136). The return values and the
//! special cases follow the C code.

use rinux_lib::task_block::TaskBlocker;

use crate::include::linux::sched::current;
use crate::kernel::sched::core::{RunqueueBlocker, sleep_until};
use crate::kernel::time::jiffies::{HZ, jiffies};

/// `MAX_SCHEDULE_TIMEOUT`: wait until something wakes the task.
pub const MAX_SCHEDULE_TIMEOUT: isize = isize::MAX;

/// The jiffies left before `expire`, or 0 once it has passed. The
/// subtraction wraps as in C, so a deadline behind `now` is negative.
fn remaining_jiffies(expire: u64, now: u64) -> isize {
    let rem = expire.wrapping_sub(now) as isize;
    if rem < 0 { 0 } else { rem }
}

/// `msecs_to_jiffies()`: milliseconds rounded up to whole ticks.
pub fn msecs_to_jiffies(msecs: u32) -> isize {
    (msecs as u64 * HZ).div_ceil(1000) as isize
}

/// `schedule_timeout()`: sleep for `timeout` jiffies. Returns the jiffies
/// that were left when the sleep ended, or 0 when it ran its course.
/// [`MAX_SCHEDULE_TIMEOUT`] blocks until a wake and returns it. A negative
/// timeout is a caller bug: it is reported, and the call returns 0 at once.
pub fn schedule_timeout(timeout: isize) -> isize {
    if timeout == MAX_SCHEDULE_TIMEOUT {
        // No timer: the task sleeps until something wakes it.
        RunqueueBlocker.block(current() as usize);
        return timeout;
    }
    if timeout < 0 {
        pr_err!(
            "schedule_timeout: wrong timeout value {:x}",
            timeout as usize
        );
        return 0;
    }

    let expire = jiffies().wrapping_add(timeout as u64);
    sleep_until(expire);
    remaining_jiffies(expire, jiffies())
}

/// `schedule_timeout_uninterruptible()`. Rinux has no signal state yet, so
/// this is the same sleep as [`schedule_timeout`].
pub fn schedule_timeout_uninterruptible(timeout: isize) -> isize {
    schedule_timeout(timeout)
}

/// `msleep()`: sleep for at least `msecs` milliseconds, looping while the
/// sleep returns early.
pub fn msleep(msecs: u32) {
    let mut timeout = msecs_to_jiffies(msecs);
    while timeout != 0 {
        timeout = schedule_timeout_uninterruptible(timeout);
    }
}

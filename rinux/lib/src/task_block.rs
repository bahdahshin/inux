// SPDX-License-Identifier: GPL-2.0
//! Blocking and waking tasks, as the sleeping wait paths need them.
//!
//! The C wait paths call `schedule()` to sleep and `try_to_wake_up()` to
//! wake a task. The lib sits below the scheduler, so it takes a
//! [`TaskBlocker`] instead. The kernel implements it over its runqueue, and
//! host tests use a double.
//!
//! The C code releases the wait-queue spinlock inside `schedule()`. A caller
//! of these functions must do the same, since `block` runs with the queue
//! borrowed.

/// The sleep and wake operations of the scheduler, on task ids.
pub trait TaskBlocker {
    /// Sleeps task `id` until [`TaskBlocker::wake`] is called for it. The
    /// kernel's `schedule()`. It may return at once if the wake has already
    /// happened.
    fn block(&mut self, id: usize);

    /// Makes task `id` runnable again. The kernel's `try_to_wake_up()`.
    fn wake(&mut self, id: usize);
}

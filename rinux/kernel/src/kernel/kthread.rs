// SPDX-License-Identifier: GPL-2.0-only
//! Kernel threads.
//!
//! A partial port of `kernel/kthread.c`. `kthread_run()` spawns a kernel
//! thread running `threadfn(arg)`. Linux routes creation through
//! `kthreadd` and a completion; with no workqueues or wait queues yet,
//! rinux creates the thread directly through [`kernel_thread`].
//!
//! Not ported: `kthread_stop()`/`kthread_should_stop()`, parking, CPU
//! binding and the `kthreadd` daemon.

use crate::include::linux::sched::TaskStruct;
use crate::kernel::sched::core::kernel_thread;

/// `kthread_run()`: create and wake a kernel thread.
///
/// Returns the task, or null on failure.
pub fn kthread_run(threadfn: extern "C" fn(u64) -> i32, arg: u64, name: &[u8]) -> *mut TaskStruct {
    kernel_thread(threadfn, arg, name)
}

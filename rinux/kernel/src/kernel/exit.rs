// SPDX-License-Identifier: GPL-2.0-only
//! Task exit.
//!
//! A partial port of `make_task_dead()` from `kernel/exit.c`. There are no
//! tasks yet: everything runs as the boot CPU's idle task, `swapper/0`
//! with PID 0, so killing the current task is always fatal.
//!
//! Copyright (C) 1991, 1992  Linus Torvalds

use crate::kernel::panic::panic;
use crate::kernel::softirq::in_interrupt;

/// `current->pid`: the idle task.
pub const CURRENT_PID: i32 = 0;
/// `current->comm`.
pub const CURRENT_COMM: &str = "swapper/0";

/// `make_task_dead()`: kills the current task after an oops.
pub fn make_task_dead(_signr: i32) -> ! {
    if in_interrupt() {
        panic(format_args!("Aiee, killing interrupt handler!"));
    }
    if CURRENT_PID == 0 {
        panic(format_args!("Attempted to kill the idle task!"));
    }
    unreachable!("there are no other tasks to kill");
}

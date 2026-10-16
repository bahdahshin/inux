// SPDX-License-Identifier: GPL-2.0
//! Core kernel code, as in Linux's `kernel/`.

#[macro_use]
pub mod printk;

pub mod cred;
pub mod exit;
pub mod futex;
pub mod irq;
pub mod kthread;
pub mod locking;
pub mod notifier;
pub mod panic;
pub mod params;
pub mod resource;
pub mod sched;
pub mod signal;
pub mod softirq;
pub mod syscalls;
pub mod time;

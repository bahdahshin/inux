// SPDX-License-Identifier: GPL-2.0
//! Log levels and the line sink that lib functions print through.
//!
//! Ported from the level part of `include/linux/printk.h` and
//! `include/linux/kern_levels.h`.
//!
//! The C code writes to the kernel log with `printk()`. This crate sits below
//! the kernel, so it cannot call that. A function that would print takes a
//! [`Sink`] instead and passes it one line at a time, without the trailing
//! newline. The kernel supplies a sink that calls its `printk()`, so there is
//! no global state here.

/// `KERN_EMERG`: the system is unusable.
pub const LOGLEVEL_EMERG: i32 = 0;
/// `KERN_ALERT`: action must be taken immediately.
pub const LOGLEVEL_ALERT: i32 = 1;
/// `KERN_CRIT`: critical conditions.
pub const LOGLEVEL_CRIT: i32 = 2;
/// `KERN_ERR`: error conditions.
pub const LOGLEVEL_ERR: i32 = 3;
/// `KERN_WARNING`: warning conditions.
pub const LOGLEVEL_WARNING: i32 = 4;
/// `KERN_NOTICE`: normal but significant conditions.
pub const LOGLEVEL_NOTICE: i32 = 5;
/// `KERN_INFO`: informational messages.
pub const LOGLEVEL_INFO: i32 = 6;
/// `KERN_DEBUG`: debug-level messages.
pub const LOGLEVEL_DEBUG: i32 = 7;

/// A line sink: receives the level and one line of text, without the newline.
pub type Sink<'a> = dyn FnMut(i32, &[u8]) + 'a;

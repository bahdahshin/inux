// SPDX-License-Identifier: GPL-2.0
//! Die notifier events.
//!
//! Ported from `arch/x86/include/asm/kdebug.h` and `struct die_args` in
//! `include/linux/kdebug.h`.

use super::ptrace::PtRegs;

// enum die_val
pub const DIE_OOPS: u64 = 1;
pub const DIE_INT3: u64 = 2;
pub const DIE_DEBUG: u64 = 3;
pub const DIE_PANIC: u64 = 4;
pub const DIE_NMI: u64 = 5;
pub const DIE_DIE: u64 = 6;
pub const DIE_KERNELDEBUG: u64 = 7;
pub const DIE_TRAP: u64 = 8;
pub const DIE_GPF: u64 = 9;
pub const DIE_CALL: u64 = 10;
pub const DIE_PAGE_FAULT: u64 = 11;
pub const DIE_NMIUNKNOWN: u64 = 12;

/// `struct die_args`: what die notifiers are told.
pub struct DieArgs {
    pub regs: *mut PtRegs,
    pub str: &'static str,
    pub err: i64,
    pub trapnr: i32,
    pub signr: i32,
}

// SAFETY: the registers pointer is only dereferenced on the CPU that took
// the exception, while the notifier chain runs.
unsafe impl Send for DieArgs {}

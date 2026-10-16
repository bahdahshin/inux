// SPDX-License-Identifier: GPL-2.0
//! Signal handling state.
//!
//! A small port of the per-task signal state of `kernel/signal.c` and
//! `include/linux/signal_types.h`: the table of installed handlers
//! (`sighand_struct`) and the blocked-signal mask (`task->blocked`), which the
//! `rt_sigaction`/`rt_sigprocmask` syscalls read and write.
//!
//! This step covers the configuration of signals only, not their delivery:
//! rinux records handlers and masks but does not yet build a signal frame on
//! the user stack or run a handler. Since there is one task, the state is a
//! pair of globals rather than a reference-counted `sighand_struct`.

use core::sync::atomic::AtomicU64;

use crate::kernel::locking::SpinLock;

/// `_NSIG`: the number of signals.
pub const NSIG: usize = 64;

/// Signals that can be neither caught nor blocked.
pub const SIGKILL: u32 = 9;
pub const SIGSTOP: u32 = 19;
/// `SIG_KERNEL_ONLY_MASK`: the bits of [`SIGKILL`]/[`SIGSTOP`].
pub const SIG_UNBLOCKABLE: u64 = (1 << (SIGKILL - 1)) | (1 << (SIGSTOP - 1));

/// `SA_SIGINFO`: the handler takes the three-argument `(sig, info, uc)` form
/// and wants a filled `siginfo`.
pub const SA_SIGINFO: u64 = 0x0000_0004;
/// `SA_RESTORER`: the action supplies its own return trampoline.
pub const SA_RESTORER: u64 = 0x0400_0000;

/// `how` values for `rt_sigprocmask`.
pub const SIG_BLOCK: u64 = 0;
pub const SIG_UNBLOCK: u64 = 1;
pub const SIG_SETMASK: u64 = 2;

/// `struct k_sigaction`: one installed signal handler. The layout matches the
/// x86-64 user `struct sigaction`: handler, flags, restorer, then the 64-bit
/// blocked-during-handler mask.
#[derive(Clone, Copy)]
pub struct KSigaction {
    pub handler: u64,
    pub flags: u64,
    pub restorer: u64,
    pub mask: u64,
}

impl KSigaction {
    const DFL: KSigaction = KSigaction {
        handler: 0,
        flags: 0,
        restorer: 0,
        mask: 0,
    };
}

/// `sighand_struct.action`: the installed handlers, indexed by signal - 1.
pub static SIGHAND: SpinLock<[KSigaction; NSIG]> = SpinLock::new([KSigaction::DFL; NSIG]);

/// `task->blocked`: the set of signals blocked for the (single) task.
pub static BLOCKED: AtomicU64 = AtomicU64::new(0);

/// `task->pending`: signals raised but not yet delivered.
pub static PENDING: AtomicU64 = AtomicU64::new(0);

/// Raises signal `sig` (1-based) on the task, as `send_signal()` does.
pub fn send_signal(sig: u32) {
    if sig >= 1 && sig as usize <= NSIG {
        PENDING.fetch_or(1 << (sig - 1), core::sync::atomic::Ordering::Relaxed);
    }
}

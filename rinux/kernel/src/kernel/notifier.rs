// SPDX-License-Identifier: GPL-2.0-only
//! Notifier chains and the die chain.
//!
//! A partial port of `kernel/notifier.c`: atomic notifier chains, kept in
//! priority order, and `notify_die()`. Linux walks the chain under RCU;
//! here the chain is copied under a lock and walked without it, so a
//! callback may unregister itself.

use crate::arch::x86::include::kdebug::DieArgs;
use crate::arch::x86::include::ptrace::PtRegs;
use crate::kernel::locking::SpinLock;
use rinux_lib::errno::{EEXIST, ENOSPC, Result};

pub const NOTIFY_DONE: i32 = 0x0000;
pub const NOTIFY_OK: i32 = 0x0001;
pub const NOTIFY_STOP_MASK: i32 = 0x8000;
pub const NOTIFY_BAD: i32 = NOTIFY_STOP_MASK | 0x0002;
pub const NOTIFY_STOP: i32 = NOTIFY_OK | NOTIFY_STOP_MASK;

const MAX_NOTIFIERS: usize = 16;

/// `struct notifier_block`.
pub struct NotifierBlock<T: 'static> {
    pub notifier_call: fn(nb: &NotifierBlock<T>, action: u64, data: &mut T) -> i32,
    /// Higher priorities run first.
    pub priority: i32,
}

/// `struct atomic_notifier_head`.
pub struct AtomicNotifierHead<T: 'static> {
    chain: SpinLock<[Option<&'static NotifierBlock<T>>; MAX_NOTIFIERS]>,
}

impl<T: 'static> AtomicNotifierHead<T> {
    pub const fn new() -> Self {
        AtomicNotifierHead {
            chain: SpinLock::new([None; MAX_NOTIFIERS]),
        }
    }

    /// `atomic_notifier_chain_register()`.
    pub fn register(&self, n: &'static NotifierBlock<T>) -> Result {
        let mut chain = self.chain.lock_irqsave();
        if chain.iter().flatten().any(|nb| core::ptr::eq(*nb, n)) {
            // Linux WARNs about double registration.
            pr_warn!("notifier callback already registered");
            return Err(EEXIST);
        }
        let len = chain.iter().flatten().count();
        if len == MAX_NOTIFIERS {
            return Err(ENOSPC);
        }
        // Insert after every entry of the same or higher priority.
        let pos = chain[..len]
            .iter()
            .flatten()
            .take_while(|nb| nb.priority >= n.priority)
            .count();
        chain.copy_within(pos..len, pos + 1);
        chain[pos] = Some(n);
        Ok(())
    }

    /// `atomic_notifier_chain_unregister()`.
    pub fn unregister(&self, n: &'static NotifierBlock<T>) -> Result {
        let mut chain = self.chain.lock_irqsave();
        let len = chain.iter().flatten().count();
        let Some(pos) = chain[..len]
            .iter()
            .position(|nb| nb.is_some_and(|nb| core::ptr::eq(nb, n)))
        else {
            return Err(rinux_lib::errno::ENOENT);
        };
        chain.copy_within(pos + 1..len, pos);
        chain[len - 1] = None;
        Ok(())
    }

    /// `atomic_notifier_call_chain()`: calls each notifier in turn until one
    /// returns a value with `NOTIFY_STOP_MASK`. Returns the last value.
    pub fn call_chain(&self, val: u64, v: &mut T) -> i32 {
        let snapshot = *self.chain.lock_irqsave();
        let mut ret = NOTIFY_DONE;
        for nb in snapshot.iter().flatten() {
            ret = (nb.notifier_call)(nb, val, v);
            if ret & NOTIFY_STOP_MASK != 0 {
                break;
            }
        }
        ret
    }
}

impl<T: 'static> Default for AtomicNotifierHead<T> {
    fn default() -> Self {
        Self::new()
    }
}

static DIE_CHAIN: AtomicNotifierHead<DieArgs> = AtomicNotifierHead::new();

/// `register_die_notifier()`.
pub fn register_die_notifier(nb: &'static NotifierBlock<DieArgs>) -> Result {
    DIE_CHAIN.register(nb)
}

/// `unregister_die_notifier()`.
pub fn unregister_die_notifier(nb: &'static NotifierBlock<DieArgs>) -> Result {
    DIE_CHAIN.unregister(nb)
}

/// `notify_die()`: tells the die chain about an exception or oops.
pub fn notify_die(
    val: u64,
    str: &'static str,
    regs: &mut PtRegs,
    err: i64,
    trap: i32,
    sig: i32,
) -> i32 {
    let mut args = DieArgs {
        regs,
        str,
        err,
        trapnr: trap,
        signr: sig,
    };
    DIE_CHAIN.call_chain(val, &mut args)
}

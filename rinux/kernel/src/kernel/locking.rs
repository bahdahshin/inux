// SPDX-License-Identifier: GPL-2.0
//! Spinlocks.
//!
//! A minimal stand-in for `kernel/locking/spinlock.c` until SMP exists: a
//! test-and-set lock that owns the data it protects. Data that interrupt
//! handlers also touch must be locked with [`SpinLock::lock_irqsave`], as
//! with `spin_lock_irqsave()` in C.

use core::cell::UnsafeCell;
use core::ops::{Deref, DerefMut};
use core::sync::atomic::{AtomicBool, Ordering};

use crate::arch::x86::include::irqflags::{local_irq_restore, local_irq_save};

/// A spinlock protecting a `T`.
pub struct SpinLock<T> {
    locked: AtomicBool,
    data: UnsafeCell<T>,
}

// SAFETY: the lock serializes all access to the data.
unsafe impl<T: Send> Sync for SpinLock<T> {}

impl<T> SpinLock<T> {
    /// A new, unlocked spinlock.
    pub const fn new(data: T) -> SpinLock<T> {
        SpinLock {
            locked: AtomicBool::new(false),
            data: UnsafeCell::new(data),
        }
    }

    /// `spin_lock()`: spins until the lock is free and takes it. Disables
    /// preemption while held, as in Linux.
    pub fn lock(&self) -> SpinLockGuard<'_, T> {
        crate::kernel::softirq::preempt_disable();
        self.acquire();
        SpinLockGuard {
            lock: self,
            irq_flags: None,
        }
    }

    /// `spin_lock_irqsave()`: disables interrupts, then takes the lock.
    /// Dropping the guard restores the interrupt state.
    pub fn lock_irqsave(&self) -> SpinLockGuard<'_, T> {
        let flags = local_irq_save();
        self.acquire();
        SpinLockGuard {
            lock: self,
            irq_flags: Some(flags),
        }
    }

    fn acquire(&self) {
        while self
            .locked
            .compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            while self.locked.load(Ordering::Relaxed) {
                core::hint::spin_loop();
            }
        }
    }

    /// Releases the lock whoever holds it, for the panic path
    /// (`bust_spinlocks()` in Linux).
    ///
    /// # Safety
    ///
    /// Only call when the holder will never run again, such as after
    /// stopping all other execution to panic.
    pub unsafe fn force_unlock(&self) {
        self.locked.store(false, Ordering::Release);
    }
}

/// Holds a [`SpinLock`] until dropped.
pub struct SpinLockGuard<'a, T> {
    lock: &'a SpinLock<T>,
    irq_flags: Option<u64>,
}

impl<T> Deref for SpinLockGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        // SAFETY: the guard holds the lock.
        unsafe { &*self.lock.data.get() }
    }
}

impl<T> DerefMut for SpinLockGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        // SAFETY: the guard holds the lock.
        unsafe { &mut *self.lock.data.get() }
    }
}

impl<T> Drop for SpinLockGuard<'_, T> {
    fn drop(&mut self) {
        self.lock.locked.store(false, Ordering::Release);
        match self.irq_flags {
            Some(flags) => local_irq_restore(flags),
            None => crate::kernel::softirq::preempt_enable(),
        }
    }
}

/// A static the boot CPU sets up before anything else can see it, such as
/// the GDT, IDT and TSS: the hardware reads them by address, so they can't
/// sit behind a lock.
pub struct RacyCell<T>(UnsafeCell<T>);

// SAFETY: users promise, through `get()`'s contract, that accesses don't
// race.
unsafe impl<T> Sync for RacyCell<T> {}

impl<T> RacyCell<T> {
    pub const fn new(value: T) -> RacyCell<T> {
        RacyCell(UnsafeCell::new(value))
    }

    /// A raw pointer to the contents.
    ///
    /// Dereferencing it is safe only while no other access can race: in
    /// practice, during single-CPU boot with the relevant interrupts off,
    /// or for reads of data that is no longer written.
    pub const fn get(&self) -> *mut T {
        self.0.get()
    }
}

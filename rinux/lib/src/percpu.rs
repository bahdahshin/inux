// SPDX-License-Identifier: GPL-2.0
//! Per-CPU variables: one slot of `T` for each CPU.
//!
//! Ported from the data layout of `include/linux/percpu-defs.h`
//! (`DEFINE_PER_CPU()`) and the accessors `per_cpu_ptr()` and
//! `this_cpu_ptr()`. C places each CPU's copy in its own memory area and
//! reaches it through a per-CPU offset. Here the slots are an array indexed by
//! CPU number; the kernel sets `N` to its CPU count. rinux runs one CPU today,
//! so `N` is 1, but the type is the same once SMP exists.
//!
//! The access rule is the one Linux relies on: a CPU touches only its own
//! slot, and does so with preemption disabled so it cannot move to another
//! CPU mid-access. [`PerCpu::get_mut`] enforces that by borrowing the whole
//! array mutably, so it is always safe. [`PerCpu::with_cpu`] allows shared use
//! from a `&self`, and is `unsafe` because the caller must uphold the rule.
//!
//! The slots live in one `UnsafeCell` so that [`PerCpu::from_array`] can be a
//! `const fn`, which a `static` needs.

use core::cell::UnsafeCell;

/// A `T` per CPU, for `N` CPUs.
pub struct PerCpu<T, const N: usize> {
    slots: UnsafeCell<[T; N]>,
}

// SAFETY: each slot is only accessed by its own CPU, under the rule in the
// module docs; `with_cpu` is `unsafe` to make the caller accept that rule.
unsafe impl<T: Send, const N: usize> Sync for PerCpu<T, N> {}

impl<T, const N: usize> PerCpu<T, N> {
    /// `DEFINE_PER_CPU()` with the starting values given in order, usable in a
    /// `static`.
    pub const fn from_array(values: [T; N]) -> Self {
        Self {
            slots: UnsafeCell::new(values),
        }
    }

    /// `DEFINE_PER_CPU()`: `init(cpu)` gives each CPU its starting value.
    pub fn new(init: impl FnMut(usize) -> T) -> Self {
        Self::from_array(core::array::from_fn(init))
    }

    /// The number of CPUs (slots).
    pub const fn cpus(&self) -> usize {
        N
    }

    /// `per_cpu_ptr()` for a `&mut` owner: the slot of `cpu`. Panics if `cpu`
    /// is out of range.
    pub fn get_mut(&mut self, cpu: usize) -> &mut T {
        &mut self.slots.get_mut()[cpu]
    }

    /// `this_cpu_ptr()`-style access from shared state: run `f` on the slot of
    /// `cpu`.
    ///
    /// # Safety
    ///
    /// `cpu` must be the calling CPU, with preemption disabled for the whole
    /// call, and no other access to that slot may run at the same time. Those
    /// are the rules `this_cpu_ptr()` depends on in C.
    pub unsafe fn with_cpu<R>(&self, cpu: usize, f: impl FnOnce(&mut T) -> R) -> R {
        // SAFETY: the caller guarantees exclusive use of this slot.
        f(unsafe { &mut (&mut *self.slots.get())[cpu] })
    }
}

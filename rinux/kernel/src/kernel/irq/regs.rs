// SPDX-License-Identifier: GPL-2.0-or-later
//! The registers of the interrupt in progress, per CPU.
//!
//! Ported from `lib/irq_regs.c` and `include/asm-generic/irq_regs.h`. C keeps
//! a `struct pt_regs *` per CPU as `__irq_regs`, and `get_irq_regs()` and
//! `set_irq_regs()` read and swap it. The slot is a `PerCpu` from rinux-lib,
//! holding the address as an integer so the slot is `Sync`; a null value means
//! no interrupt frame is current.
//!
//! rinux runs one CPU, and the slot is read and written only from interrupt
//! entry and exit with interrupts disabled, so the `unsafe` slot access below
//! meets the per-CPU rule.

use core::sync::atomic::AtomicUsize;

use rinux_lib::percpu::PerCpu;

use crate::arch::x86::include::ptrace::PtRegs;

/// `__irq_regs`: the current interrupt frame, as an address (0 for none).
static IRQ_REGS: PerCpu<AtomicUsize, 1> = PerCpu::from_array([AtomicUsize::new(0)]);

/// `get_irq_regs()`: the frame of the interrupt in progress, if any.
pub fn get_irq_regs() -> Option<*mut PtRegs> {
    // SAFETY: one CPU, and interrupts are disabled around every access (the
    // slot is only used from interrupt entry and exit).
    let addr = unsafe {
        IRQ_REGS.with_cpu(0, |slot| slot.load(core::sync::atomic::Ordering::Relaxed))
    };
    if addr == 0 {
        None
    } else {
        Some(addr as *mut PtRegs)
    }
}

/// `set_irq_regs()`: make `new_regs` the current frame, returning the old one.
pub fn set_irq_regs(new_regs: Option<*mut PtRegs>) -> Option<*mut PtRegs> {
    let new_addr = new_regs.map_or(0, |p| p as usize);
    // SAFETY: as for `get_irq_regs`.
    let old = unsafe {
        IRQ_REGS.with_cpu(0, |slot| {
            slot.swap(new_addr, core::sync::atomic::Ordering::Relaxed)
        })
    };
    if old == 0 { None } else { Some(old as *mut PtRegs) }
}

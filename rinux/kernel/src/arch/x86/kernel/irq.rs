// SPDX-License-Identifier: GPL-2.0
//! Interrupt dispatch.
//!
//! A partial port of `arch/x86/kernel/irq.c`: `common_interrupt()` maps
//! the vector to an interrupt through `vector_irq` and runs it. The local
//! APIC acknowledgement, `irq_stat` accounting and `/proc/interrupts`
//! come with the APIC.

use core::sync::atomic::{AtomicI32, AtomicU64, Ordering};

use crate::arch::x86::include::irq_vectors::NR_VECTORS;
use crate::arch::x86::include::ptrace::PtRegs;
use crate::kernel::irq::generic_handle_irq;
use crate::kernel::softirq::{irq_enter, irq_exit};

/// `VECTOR_UNUSED`.
pub const VECTOR_UNUSED: i32 = -1;

/// `vector_irq`: the interrupt behind each vector on this CPU.
static VECTOR_IRQ: [AtomicI32; NR_VECTORS as usize] =
    [const { AtomicI32::new(VECTOR_UNUSED) }; NR_VECTORS as usize];

/// Vectors already reported as unhandled: a stand-in for
/// `pr_emerg_ratelimited()`.
static REPORTED: [AtomicU64; 4] = [const { AtomicU64::new(0) }; 4];

/// Routes `vector` to `irq`.
pub fn set_vector_irq(vector: u32, irq: i32) {
    VECTOR_IRQ[vector as usize].store(irq, Ordering::Relaxed);
}

/// Reports a vector once.
fn first_report(vector: u8) -> bool {
    let bit = 1u64 << (vector % 64);
    REPORTED[vector as usize / 64].fetch_or(bit, Ordering::Relaxed) & bit == 0
}

/// `common_interrupt()`: an external interrupt. The entry stub pushed the
/// vector in place of the error code.
pub extern "C" fn common_interrupt(_regs: &mut PtRegs, error_code: u64) {
    let vector = error_code as u8;
    irq_enter();
    // call_irq_handler()
    let irq = VECTOR_IRQ[vector as usize].load(Ordering::Relaxed);
    let handled = irq >= 0 && generic_handle_irq(irq as u32);
    if !handled && first_report(vector) {
        pr_emerg!(
            "common_interrupt: {}.{} No irq handler for vector",
            0,
            vector
        );
    }
    irq_exit();
    // Preempt on return from the interrupt if a reschedule is due.
    crate::kernel::sched::core::preempt_on_irq_return();
}

/// `spurious_interrupt()`: a system vector nothing is set up for.
pub extern "C" fn spurious_interrupt(_regs: &mut PtRegs, error_code: u64) {
    let vector = error_code as u8;
    // handle_spurious_interrupt()
    if first_report(vector) {
        pr_info!(
            "spurious APIC interrupt through vector {:02x} on CPU#{}, should never happen.",
            vector,
            0
        );
    }
}

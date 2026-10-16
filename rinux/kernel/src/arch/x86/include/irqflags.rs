// SPDX-License-Identifier: GPL-2.0
//! Interrupt flag and halt helpers.
//!
//! Ported from `arch/x86/include/asm/irqflags.h`.

use core::arch::asm;

use super::processor_flags::X86_EFLAGS_IF;

/// `native_save_fl()`: the current RFLAGS.
#[inline]
pub fn native_save_fl() -> u64 {
    let flags: u64;
    // SAFETY: pushf/pop only reads RFLAGS.
    unsafe {
        asm!("pushfq", "popq {}", out(reg) flags, options(att_syntax, nomem, preserves_flags))
    }
    flags
}

/// `native_irq_disable()`: `cli`.
#[inline]
pub fn local_irq_disable() {
    // SAFETY: clearing IF only masks interrupts.
    unsafe { asm!("cli", options(nomem, nostack)) }
}

/// `native_irq_enable()`: `sti`.
#[inline]
pub fn local_irq_enable() {
    // SAFETY: the IDT and interrupt controllers are set up before anyone
    // enables interrupts.
    unsafe { asm!("sti", options(nomem, nostack)) }
}

/// `arch_irqs_disabled_flags()`.
#[inline]
pub fn irqs_disabled_flags(flags: u64) -> bool {
    flags & X86_EFLAGS_IF == 0
}

/// `irqs_disabled()`.
#[inline]
pub fn irqs_disabled() -> bool {
    irqs_disabled_flags(native_save_fl())
}

/// `local_irq_save()`: disables interrupts and returns the old flags.
#[inline]
pub fn local_irq_save() -> u64 {
    let flags = native_save_fl();
    local_irq_disable();
    flags
}

/// `local_irq_restore()`: re-enables interrupts if `flags` had them on.
#[inline]
pub fn local_irq_restore(flags: u64) {
    if !irqs_disabled_flags(flags) {
        local_irq_enable();
    }
}

/// `native_halt()`: `hlt` until the next interrupt.
#[inline]
pub fn native_halt() {
    // SAFETY: hlt only waits.
    unsafe { asm!("hlt", options(nomem, nostack)) }
}

/// `native_safe_halt()`: enable interrupts and halt in one step, so an
/// interrupt can't slip in between and leave the CPU asleep.
#[inline]
pub fn native_safe_halt() {
    // SAFETY: as for local_irq_enable() and native_halt().
    unsafe { asm!("sti; hlt", options(nomem, nostack)) }
}

/// Stops this CPU for good: interrupts off, then halt forever.
pub fn halt_forever() -> ! {
    loop {
        local_irq_disable();
        native_halt();
    }
}

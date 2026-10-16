// SPDX-License-Identifier: GPL-2.0
//! Interrupt initialization.
//!
//! A partial port of `arch/x86/kernel/irqinit.c`: the legacy PIC and the
//! IDT gates for external interrupts. IO-APIC and local APIC setup come
//! later; until then the 8259s deliver every interrupt.

use crate::arch::x86::include::i8259::NR_IRQS_LEGACY;
use crate::arch::x86::kernel::i8259::{I8259A_CHIP, init_8259a, isa_irq_vector};
use crate::arch::x86::kernel::idt::idt_setup_apic_and_irq_gates;
use crate::arch::x86::kernel::irq::set_vector_irq;
use crate::kernel::irq::{IRQ_NONE, IrqReturn, irq_set_chip, request_irq};

/// `no_action()`: the handler of the cascade interrupt.
fn no_action(_irq: u32) -> IrqReturn {
    IRQ_NONE
}

/// `init_ISA_irqs()`: the PICs and their 16 interrupts.
fn init_isa_irqs() {
    init_8259a(false);
    for i in 0..NR_IRQS_LEGACY {
        irq_set_chip(i, &I8259A_CHIP);
    }
}

/// `init_IRQ()`.
pub fn init_irq() {
    init_isa_irqs();

    // The legacy IRQs are routed through the PIC on this CPU.
    for i in 0..NR_IRQS_LEGACY {
        set_vector_irq(isa_irq_vector(i), i as i32);
    }

    // native_init_IRQ()
    idt_setup_apic_and_irq_gates();

    // Without an IO-APIC, the slave PIC reaches the CPU through IRQ2.
    if request_irq(2, no_action, "cascade").is_err() {
        pr_err!("Failed to register cascade interrupt");
    }
}

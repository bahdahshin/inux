// SPDX-License-Identifier: GPL-2.0
//! Interrupt descriptors, chips and handlers.
//!
//! A partial port of `kernel/irq/irqdesc.c`, `kernel/irq/chip.c` and
//! `kernel/irq/manage.c`: a fixed table of the 16 legacy interrupts, each
//! with its controller (`struct irq_chip`) and one handler, delivered with
//! the level-triggered flow the 8259 uses. Shared handlers, threaded
//! handlers, affinity and `/proc/interrupts` come later.

use core::sync::atomic::{AtomicU64, Ordering};

use rinux_lib::errno::{EBUSY, EINVAL, Result};

use crate::arch::x86::include::i8259::NR_IRQS_LEGACY;
use crate::kernel::locking::SpinLock;

pub mod regs;

/// `NR_IRQS` for now: the legacy ISA interrupts.
pub const NR_IRQS: usize = NR_IRQS_LEGACY as usize;

/// `irqreturn_t`.
pub type IrqReturn = u32;
/// `IRQ_NONE`: the interrupt was not from this device.
pub const IRQ_NONE: IrqReturn = 0;
/// `IRQ_HANDLED`.
pub const IRQ_HANDLED: IrqReturn = 1;

/// `irq_handler_t`.
pub type IrqHandler = fn(irq: u32) -> IrqReturn;

/// `struct irq_chip`: how to control one interrupt controller.
pub struct IrqChip {
    pub name: &'static str,
    pub irq_mask: fn(irq: u32),
    pub irq_unmask: fn(irq: u32),
    pub irq_mask_ack: fn(irq: u32),
}

#[derive(Clone, Copy)]
struct IrqDesc {
    chip: Option<&'static IrqChip>,
    handler: Option<IrqHandler>,
    name: &'static str,
}

static IRQ_DESC: SpinLock<[IrqDesc; NR_IRQS]> = SpinLock::new(
    [IrqDesc {
        chip: None,
        handler: None,
        name: "",
    }; NR_IRQS],
);

/// `kstat_irqs`: interrupts delivered per IRQ.
static IRQ_COUNT: [AtomicU64; NR_IRQS] = [const { AtomicU64::new(0) }; NR_IRQS];

/// `irq_set_chip_and_handler()`: attaches an interrupt to its controller.
pub fn irq_set_chip(irq: u32, chip: &'static IrqChip) {
    if let Some(desc) = IRQ_DESC.lock_irqsave().get_mut(irq as usize) {
        desc.chip = Some(chip);
    }
}

/// `request_irq()`: installs `handler` for `irq` and unmasks it.
pub fn request_irq(irq: u32, handler: IrqHandler, name: &'static str) -> Result {
    let chip = {
        let mut table = IRQ_DESC.lock_irqsave();
        let desc = table.get_mut(irq as usize).ok_or(EINVAL)?;
        if desc.handler.is_some() {
            return Err(EBUSY);
        }
        let chip = desc.chip.ok_or(EINVAL)?;
        desc.handler = Some(handler);
        desc.name = name;
        chip
    };
    // irq_startup()
    (chip.irq_unmask)(irq);
    Ok(())
}

/// `kstat_irqs()`: how many times `irq` was delivered.
pub fn kstat_irqs(irq: u32) -> u64 {
    IRQ_COUNT
        .get(irq as usize)
        .map_or(0, |c| c.load(Ordering::Relaxed))
}

/// `handle_level_irq()`: mask and acknowledge, run the handler, unmask.
///
/// Returns false if there is no such interrupt.
pub fn generic_handle_irq(irq: u32) -> bool {
    let Some(desc) = IRQ_DESC.lock_irqsave().get(irq as usize).copied() else {
        return false;
    };
    let Some(chip) = desc.chip else {
        return false;
    };
    (chip.irq_mask_ack)(irq);
    // irq_can_handle(): with no handler the line stays masked.
    let Some(handler) = desc.handler else {
        return true;
    };
    IRQ_COUNT[irq as usize].fetch_add(1, Ordering::Relaxed);
    handler(irq);
    (chip.irq_unmask)(irq);
    true
}

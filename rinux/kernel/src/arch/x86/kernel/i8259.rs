// SPDX-License-Identifier: GPL-2.0
//! The 8259A interrupt controller pair.
//!
//! A partial port of `arch/x86/kernel/i8259.c`: initialization, masking
//! and the specific-EOI acknowledge used with the level-triggered flow.
//! Suspend/resume and ELCR trigger setup are not ported.

use core::sync::atomic::{AtomicU32, Ordering};

use crate::arch::x86::include::i8259::*;
use crate::arch::x86::include::io::{inb, outb};
use crate::arch::x86::include::irq_vectors::FIRST_EXTERNAL_VECTOR;
use crate::arch::x86::lib::delay::udelay;
use crate::kernel::irq::IrqChip;
use crate::kernel::locking::SpinLock;

/// `ISA_IRQ_VECTOR()`: the vector the legacy IRQs are remapped to (0x30+).
pub const fn isa_irq_vector(irq: u32) -> u32 {
    ((FIRST_EXTERNAL_VECTOR + 16) & !15) + irq
}

/// `i8259A_lock`.
static I8259A_LOCK: SpinLock<()> = SpinLock::new(());

/// `cached_irq_mask`: both controllers' masks; all masked at boot.
static CACHED_IRQ_MASK: AtomicU32 = AtomicU32::new(0xffff);

fn cached_master_mask() -> u8 {
    CACHED_IRQ_MASK.load(Ordering::Relaxed) as u8
}

fn cached_slave_mask() -> u8 {
    (CACHED_IRQ_MASK.load(Ordering::Relaxed) >> 8) as u8
}

/// Port writes to the PICs, which nothing else owns.
fn pic_out(value: u8, port: u16) {
    // SAFETY: the PIC ports belong to this driver.
    unsafe { outb(value, port) }
}

fn pic_in(port: u16) -> u8 {
    // SAFETY: the PIC ports belong to this driver.
    unsafe { inb(port) }
}

/// `outb_pic()`: some chipsets need a microsecond between PIC accesses.
fn outb_pic(value: u8, port: u16) {
    pic_out(value, port);
    udelay(2);
}

fn write_mask(irq: u32) {
    if irq & 8 != 0 {
        pic_out(cached_slave_mask(), PIC_SLAVE_IMR);
    } else {
        pic_out(cached_master_mask(), PIC_MASTER_IMR);
    }
}

/// `mask_8259A_irq()`.
fn mask_8259a_irq(irq: u32) {
    let _guard = I8259A_LOCK.lock_irqsave();
    CACHED_IRQ_MASK.fetch_or(1 << irq, Ordering::Relaxed);
    write_mask(irq);
}

/// `unmask_8259A_irq()`.
fn unmask_8259a_irq(irq: u32) {
    let _guard = I8259A_LOCK.lock_irqsave();
    CACHED_IRQ_MASK.fetch_and(!(1 << irq), Ordering::Relaxed);
    write_mask(irq);
}

/// `i8259A_irq_real()`: is the IRQ really in service, according to the ISR?
fn i8259a_irq_real(irq: u32) -> bool {
    let irqmask = 1u32 << irq;
    if irq < 8 {
        pic_out(0x0b, PIC_MASTER_CMD); // ISR register
        let value = pic_in(PIC_MASTER_CMD) as u32 & irqmask;
        pic_out(0x0a, PIC_MASTER_CMD); // back to the IRR register
        return value != 0;
    }
    pic_out(0x0b, PIC_SLAVE_CMD);
    let value = pic_in(PIC_SLAVE_CMD) as u32 & (irqmask >> 8);
    pic_out(0x0a, PIC_SLAVE_CMD);
    value != 0
}

/// `mask_and_ack_8259A()`: mask the IRQ and send a specific EOI.
///
/// IRQ7 and IRQ15 can arrive spurious even when masked. Linux reports the
/// first spurious IRQ at debug level with `printk_deferred()`; rinux has no
/// deferred printk yet, and printing from here could deadlock, so it only
/// handles them.
fn mask_and_ack_8259a(irq: u32) {
    let irqmask = 1u32 << irq;
    let _guard = I8259A_LOCK.lock_irqsave();

    if CACHED_IRQ_MASK.load(Ordering::Relaxed) & irqmask != 0 {
        // Possibly spurious; handle it either way, as Linux does.
        let _ = i8259a_irq_real(irq);
    }
    CACHED_IRQ_MASK.fetch_or(irqmask, Ordering::Relaxed);

    if irq & 8 != 0 {
        pic_in(PIC_SLAVE_IMR); // dummy read
        pic_out(cached_slave_mask(), PIC_SLAVE_IMR);
        // Specific EOI to the slave, then to master IR2.
        pic_out(0x60 + (irq & 7) as u8, PIC_SLAVE_CMD);
        pic_out(0x60 + PIC_CASCADE_IR, PIC_MASTER_CMD);
    } else {
        pic_in(PIC_MASTER_IMR); // dummy read
        pic_out(cached_master_mask(), PIC_MASTER_IMR);
        // Specific EOI to the master.
        pic_out(0x60 + irq as u8, PIC_MASTER_CMD);
    }
}

/// `i8259A_chip`.
pub static I8259A_CHIP: IrqChip = IrqChip {
    name: "XT-PIC",
    irq_mask: mask_8259a_irq,
    irq_unmask: unmask_8259a_irq,
    irq_mask_ack: mask_and_ack_8259a,
};

/// `init_8259A()`: remap both PICs above the exception vectors.
pub fn init_8259a(auto_eoi: bool) {
    let _guard = I8259A_LOCK.lock_irqsave();

    pic_out(0xff, PIC_MASTER_IMR); // mask all of 8259A-1

    outb_pic(0x11, PIC_MASTER_CMD); // ICW1: select 8259A-1 init
    // ICW2: 8259A-1 IR0-7 mapped to ISA_IRQ_VECTOR(0)
    outb_pic(isa_irq_vector(0) as u8, PIC_MASTER_IMR);
    // 8259A-1 (the master) has a slave on IR2
    outb_pic(1 << PIC_CASCADE_IR, PIC_MASTER_IMR);
    if auto_eoi {
        outb_pic(MASTER_ICW4_DEFAULT | PIC_ICW4_AEOI, PIC_MASTER_IMR);
    } else {
        outb_pic(MASTER_ICW4_DEFAULT, PIC_MASTER_IMR);
    }

    outb_pic(0x11, PIC_SLAVE_CMD); // ICW1: select 8259A-2 init
    // ICW2: 8259A-2 IR0-7 mapped to ISA_IRQ_VECTOR(8)
    outb_pic(isa_irq_vector(8) as u8, PIC_SLAVE_IMR);
    // 8259A-2 is a slave on master's IR2
    outb_pic(PIC_CASCADE_IR, PIC_SLAVE_IMR);
    outb_pic(SLAVE_ICW4_DEFAULT, PIC_SLAVE_IMR);

    udelay(100); // wait for 8259A to initialize

    pic_out(cached_master_mask(), PIC_MASTER_IMR); // restore master IRQ mask
    pic_out(cached_slave_mask(), PIC_SLAVE_IMR); // restore slave IRQ mask
}

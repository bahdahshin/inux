// SPDX-License-Identifier: GPL-2.0
//! The 8259A programmable interrupt controller: ports and init words.
//!
//! Ported from `arch/x86/include/asm/i8259.h`.

pub const PIC_MASTER_CMD: u16 = 0x20;
pub const PIC_MASTER_IMR: u16 = 0x21;
pub const PIC_MASTER_ISR: u16 = PIC_MASTER_CMD;
pub const PIC_MASTER_POLL: u16 = PIC_MASTER_ISR;
pub const PIC_MASTER_OCW3: u16 = PIC_MASTER_ISR;
pub const PIC_SLAVE_CMD: u16 = 0xa0;
pub const PIC_SLAVE_IMR: u16 = 0xa1;
pub const PIC_ELCR1: u16 = 0x4d0;
pub const PIC_ELCR2: u16 = 0x4d1;

/// The slave is wired to the master's IR2.
pub const PIC_CASCADE_IR: u8 = 2;
pub const MASTER_ICW4_DEFAULT: u8 = 0x01;
pub const SLAVE_ICW4_DEFAULT: u8 = 0x01;
pub const PIC_ICW4_AEOI: u8 = 2;

/// `NR_IRQS_LEGACY`: the 16 ISA interrupts.
pub const NR_IRQS_LEGACY: u32 = 16;

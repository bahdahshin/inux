// SPDX-License-Identifier: GPL-2.0
//! `arch/x86/kernel`.

pub mod alternative;
pub mod cpu;
pub mod dumpstack;
pub mod e820;
pub mod early_printk;
pub mod head64;
pub mod i8259;
pub mod idt;
pub mod irq;
pub mod irqinit;
pub mod process;
pub mod process_64;
pub mod setup;
pub mod signal;
pub mod syscall_64;
pub mod time;
pub mod traps;
pub mod tsc;

// SPDX-License-Identifier: GPL-2.0-only
//! Register dumps.
//!
//! A partial port of `__show_regs()` from `arch/x86/kernel/process_64.c`.
//!
//! Copyright (C) 1995  Linus Torvalds

use core::arch::asm;

use crate::arch::x86::include::msr_index::{MSR_FS_BASE, MSR_GS_BASE, MSR_KERNEL_GS_BASE};
use crate::arch::x86::include::processor::{rdmsr, read_cr0, read_cr2, read_cr3, read_cr4};
use crate::arch::x86::include::ptrace::PtRegs;
use crate::arch::x86::kernel::dumpstack::show_iret_regs;
use crate::arch::x86::kernel::traps::DR6_RESERVED;
use crate::kernel::printk::LOGLEVEL_DEFAULT;

/// `DR7_FIXED_1`: DR7 bit 10 always reads as 1.
const DR7_FIXED_1: u64 = 0x400;

/// `enum show_regs_mode`.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ShowRegsMode {
    Short,
    User,
    All,
}

macro_rules! savesegment {
    ($seg:literal) => {{
        let v: u32;
        // SAFETY: reading a segment register has no side effects.
        unsafe { asm!(concat!("movl %", $seg, ", {0:e}"), out(reg) v, options(att_syntax, nomem, nostack, preserves_flags)) }
        v
    }};
}

macro_rules! get_debugreg {
    ($n:literal) => {{
        let v: u64;
        // SAFETY: reading a debug register at CPL0 has no side effects.
        unsafe { asm!(concat!("mov %dr", $n, ", {}"), out(reg) v, options(att_syntax, nomem, nostack, preserves_flags)) }
        v
    }};
}

/// `__show_regs()`.
pub fn __show_regs(regs: &PtRegs, mode: ShowRegsMode) {
    show_iret_regs(regs);

    printk!(
        LOGLEVEL_DEFAULT,
        "RAX: {:016x} RBX: {:016x} RCX: {:016x}",
        regs.ax,
        regs.bx,
        regs.cx
    );
    printk!(
        LOGLEVEL_DEFAULT,
        "RDX: {:016x} RSI: {:016x} RDI: {:016x}",
        regs.dx,
        regs.si,
        regs.di
    );
    printk!(
        LOGLEVEL_DEFAULT,
        "RBP: {:016x} R08: {:016x} R09: {:016x}",
        regs.bp,
        regs.r8,
        regs.r9
    );
    printk!(
        LOGLEVEL_DEFAULT,
        "R10: {:016x} R11: {:016x} R12: {:016x}",
        regs.r10,
        regs.r11,
        regs.r12
    );
    printk!(
        LOGLEVEL_DEFAULT,
        "R13: {:016x} R14: {:016x} R15: {:016x}",
        regs.r13,
        regs.r14,
        regs.r15
    );

    if mode == ShowRegsMode::Short {
        return;
    }

    // SAFETY: the FS/GS base MSRs exist on every x86-64 CPU.
    let (fs, gs, shadowgs) = unsafe {
        (
            rdmsr(MSR_FS_BASE as u32),
            rdmsr(MSR_GS_BASE as u32),
            rdmsr(MSR_KERNEL_GS_BASE as u32),
        )
    };

    if mode == ShowRegsMode::User {
        printk!(LOGLEVEL_DEFAULT, "FS:  {:016x} GS:  {:016x}", fs, shadowgs);
        return;
    }

    let ds = savesegment!("ds");
    let es = savesegment!("es");
    let fsindex = savesegment!("fs");
    let gsindex = savesegment!("gs");

    printk!(
        LOGLEVEL_DEFAULT,
        "FS:  {:016x}({:04x}) GS:{:016x}({:04x}) knlGS:{:016x}",
        fs,
        fsindex,
        gs,
        gsindex,
        shadowgs
    );
    printk!(
        LOGLEVEL_DEFAULT,
        "CS:  {:04x} DS: {:04x} ES: {:04x} CR0: {:016x}",
        regs.cs as u16,
        ds,
        es,
        read_cr0()
    );
    printk!(
        LOGLEVEL_DEFAULT,
        "CR2: {:016x} CR3: {:016x} CR4: {:016x}",
        read_cr2(),
        read_cr3(),
        read_cr4()
    );

    let (d0, d1, d2, d3) = (
        get_debugreg!("0"),
        get_debugreg!("1"),
        get_debugreg!("2"),
        get_debugreg!("3"),
    );
    let (d6, d7) = (get_debugreg!("6"), get_debugreg!("7"));
    // Only print the debug registers when they are not in their default state.
    if !(d0 == 0 && d1 == 0 && d2 == 0 && d3 == 0 && d6 == DR6_RESERVED && d7 == DR7_FIXED_1) {
        printk!(
            LOGLEVEL_DEFAULT,
            "DR0: {:016x} DR1: {:016x} DR2: {:016x}",
            d0,
            d1,
            d2
        );
        printk!(
            LOGLEVEL_DEFAULT,
            "DR3: {:016x} DR6: {:016x} DR7: {:016x}",
            d3,
            d6,
            d7
        );
    }
}

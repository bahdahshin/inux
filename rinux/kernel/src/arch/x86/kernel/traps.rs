// SPDX-License-Identifier: GPL-2.0
//! Exception handlers.
//!
//! A partial port of `arch/x86/kernel/traps.c`, plus `exc_nmi()` from
//! `nmi.c` and the no-MCE `exc_machine_check()` from `cpu/mce/core.c`.
//! There is no user mode yet, so faults are handled the way Linux handles
//! them in the kernel: let the die notifiers look, then oops. Exception
//! table fixups, `WARN()` via `ud2`, kprobes, and vm86 are not ported.
//!
//! Copyright (C) 1991, 1992  Linus Torvalds
//! Copyright (C) 2000, 2001, 2002 Andi Kleen, SuSE Labs

use core::arch::asm;

use crate::arch::x86::include::io::inb;
use crate::arch::x86::include::kdebug::{DIE_DEBUG, DIE_GPF, DIE_INT3, DIE_NMIUNKNOWN, DIE_TRAP};
use crate::arch::x86::include::processor_flags::X86_EFLAGS_TF;
use crate::arch::x86::include::ptrace::{PtRegs, user_mode};
use crate::arch::x86::include::trapnr::*;
use crate::arch::x86::kernel::cpu::common::cpu_init_exception_handling;
use crate::arch::x86::kernel::dumpstack::{die, die_addr};
use crate::arch::x86::kernel::idt::idt_setup_traps;
use crate::kernel::notifier::{NOTIFY_STOP, notify_die};
use crate::kernel::panic::panic;

// Signal numbers, arch/x86/include/uapi/asm/signal.h.
pub const SIGILL: i32 = 4;
pub const SIGTRAP: i32 = 5;
pub const SIGBUS: i32 = 7;
pub const SIGFPE: i32 = 8;
pub const SIGKILL: i32 = 9;
pub const SIGSEGV: i32 = 11;

/// `do_trap()`: a fault the kernel has no fixup for.
fn do_trap(trapnr: u32, _signr: i32, str: &'static str, regs: &mut PtRegs, error_code: i64) {
    if user_mode(regs) {
        // force_sig() comes with user space; nothing runs there yet.
        pr_err!("rinux: trap {trapnr} ({str}) in user mode, signals are not ported");
    }
    die(str, regs, error_code);
}

/// `do_error_trap()`.
fn do_error_trap(regs: &mut PtRegs, error_code: i64, str: &'static str, trapnr: u32, signr: i32) {
    if notify_die(DIE_TRAP, str, regs, error_code, trapnr as i32, signr) != NOTIFY_STOP {
        do_trap(trapnr, signr, str, regs, error_code);
    }
}

pub extern "C" fn exc_divide_error(regs: &mut PtRegs) {
    do_error_trap(regs, 0, "divide error", X86_TRAP_DE, SIGFPE);
}

pub extern "C" fn exc_overflow(regs: &mut PtRegs) {
    do_error_trap(regs, 0, "overflow", X86_TRAP_OF, SIGSEGV);
}

/// `exc_invalid_op()`. Linux first checks for the `ud2` of `BUG()` and
/// `WARN()`; Rust code reports bugs by panicking instead.
pub extern "C" fn exc_invalid_op(regs: &mut PtRegs) {
    do_error_trap(regs, 0, "invalid opcode", X86_TRAP_UD, SIGILL);
}

pub extern "C" fn exc_coproc_segment_overrun(regs: &mut PtRegs) {
    do_error_trap(
        regs,
        0,
        "coprocessor segment overrun",
        X86_TRAP_OLD_MF,
        SIGFPE,
    );
}

pub extern "C" fn exc_invalid_tss(regs: &mut PtRegs, error_code: u64) {
    do_error_trap(regs, error_code as i64, "invalid TSS", X86_TRAP_TS, SIGSEGV);
}

pub extern "C" fn exc_segment_not_present(regs: &mut PtRegs, error_code: u64) {
    do_error_trap(
        regs,
        error_code as i64,
        "segment not present",
        X86_TRAP_NP,
        SIGBUS,
    );
}

pub extern "C" fn exc_stack_segment(regs: &mut PtRegs, error_code: u64) {
    do_error_trap(
        regs,
        error_code as i64,
        "stack segment",
        X86_TRAP_SS,
        SIGBUS,
    );
}

/// `exc_alignment_check()`: in the kernel this means a split lock.
pub extern "C" fn exc_alignment_check(regs: &mut PtRegs, error_code: u64) {
    if !user_mode(regs) {
        die("Split lock detected", regs, error_code as i64);
    }
    do_error_trap(
        regs,
        error_code as i64,
        "alignment check",
        X86_TRAP_AC,
        SIGBUS,
    );
}

/// `exc_double_fault()`: runs on its own IST stack and never returns.
pub extern "C" fn exc_double_fault(regs: &mut PtRegs, error_code: u64) {
    pr_emerg!("PANIC: double fault, error_code: {:#x}", error_code);
    die("double fault", regs, error_code as i64);
    panic(format_args!("Machine halted."));
}

pub extern "C" fn exc_bounds(regs: &mut PtRegs) {
    if notify_die(DIE_TRAP, "bounds", regs, 0, X86_TRAP_BR as i32, SIGSEGV) == NOTIFY_STOP {
        return;
    }
    if !user_mode(regs) {
        die("bounds", regs, 0);
    }
    do_trap(X86_TRAP_BR, SIGSEGV, "bounds", regs, 0);
}

const GPFSTR: &str = "general protection fault";

/// `exc_general_protection()`.
///
/// Linux decodes the faulting instruction to report a non-canonical
/// address ("probably for non-canonical address ..."); the instruction
/// decoder isn't ported, so rinux gives the plain message.
pub extern "C" fn exc_general_protection(regs: &mut PtRegs, error_code: u64) {
    let desc = if error_code != 0 {
        "segment-related general protection fault"
    } else {
        GPFSTR
    };
    // gp_try_fixup_and_notify()
    if notify_die(
        DIE_GPF,
        desc,
        regs,
        error_code as i64,
        X86_TRAP_GP as i32,
        SIGSEGV,
    ) == NOTIFY_STOP
    {
        return;
    }
    if user_mode(regs) {
        do_trap(X86_TRAP_GP, SIGSEGV, desc, regs, error_code as i64);
        return;
    }
    die_addr(desc, regs, error_code as i64, 0);
}

/// `do_int3()`: gives the die notifiers (kprobes, kgdb, selftests) a look.
fn do_int3(regs: &mut PtRegs) -> bool {
    notify_die(DIE_INT3, "int3", regs, 0, X86_TRAP_BP as i32, SIGTRAP) == NOTIFY_STOP
}

/// `exc_int3()`.
pub extern "C" fn exc_int3(regs: &mut PtRegs) {
    if user_mode(regs) {
        do_trap(X86_TRAP_BP, SIGTRAP, "int3", regs, 0);
        return;
    }
    if !do_int3(regs) {
        die("int3", regs, 0);
    }
}

/// `DR_STEP`: single-step in DR6.
const DR_STEP: u64 = 0x4000;
/// `DR6_RESERVED`.
pub const DR6_RESERVED: u64 = 0xffff_0ff0;

/// `exc_debug()`: a debug trap. With no debugger, only notifiers care; a
/// stray single-step is cleared with a warning.
pub extern "C" fn exc_debug(regs: &mut PtRegs) {
    let dr6: u64;
    // SAFETY: DR6 is readable and writable at CPL0.
    unsafe {
        asm!("mov %dr6, {}", out(reg) dr6, options(att_syntax, nomem, nostack));
        asm!("mov {}, %dr6", in(reg) DR6_RESERVED, options(att_syntax, nomem, nostack));
    }
    if notify_die(DIE_DEBUG, "debug", regs, dr6 as i64, 0, SIGTRAP) == NOTIFY_STOP {
        return;
    }
    if rinux_lib::warn_on_once!(dr6 & DR_STEP != 0) {
        regs.flags &= !X86_EFLAGS_TF;
    }
}

/// `NMI_REASON_PORT`: system control port B.
const NMI_REASON_PORT: u16 = 0x61;

/// `exc_nmi()`: with no NMI sources registered, every NMI is unknown.
pub extern "C" fn exc_nmi(regs: &mut PtRegs) {
    // SAFETY: reading system control port B has no side effects.
    let reason = unsafe { inb(NMI_REASON_PORT) };
    if notify_die(DIE_NMIUNKNOWN, "nmi", regs, reason as i64, 2, 0) == NOTIFY_STOP {
        return;
    }
    // unknown_nmi_error()
    pr_emerg!(
        "Uhhuh. NMI received for unknown reason {:02x} on CPU {}.",
        reason,
        0
    );
    pr_emerg!("Dazed and confused, but trying to continue");
}

/// `exc_machine_check()` without `CONFIG_X86_MCE`: `unexpected_machine_check()`.
pub extern "C" fn exc_machine_check(_regs: &mut PtRegs) {
    pr_err!("CPU#{}: Unexpected int18 (Machine Check)", 0);
}

/// `exc_device_not_available()`: no task ever has a lazily restored FPU.
pub extern "C" fn exc_device_not_available(regs: &mut PtRegs) {
    die("unexpected #NM exception", regs, 0);
}

/// `math_error()`.
fn math_error(regs: &mut PtRegs, trapnr: u32) {
    let str = if trapnr == X86_TRAP_MF {
        "fpu exception"
    } else {
        "simd exception"
    };
    if !user_mode(regs) {
        if notify_die(DIE_TRAP, str, regs, 0, trapnr as i32, SIGFPE) != NOTIFY_STOP {
            die(str, regs, 0);
        }
        return;
    }
    do_trap(trapnr, SIGFPE, str, regs, 0);
}

pub extern "C" fn exc_coprocessor_error(regs: &mut PtRegs) {
    math_error(regs, X86_TRAP_MF);
}

pub extern "C" fn exc_simd_coprocessor_error(regs: &mut PtRegs) {
    math_error(regs, X86_TRAP_XF);
}

/// `exc_spurious_interrupt_bug()`: a Pentium Pro erratum can deliver
/// spurious interrupts on vector 15. Ignore them.
pub extern "C" fn exc_spurious_interrupt_bug(_regs: &mut PtRegs) {}

/// `trap_init()`: TSS and interrupt stacks first, so the IST gates work.
pub fn trap_init() {
    cpu_init_exception_handling();
    idt_setup_traps();
    // Enable the SYSCALL fast path for user space.
    crate::arch::x86::kernel::syscall_64::syscall_init();
}

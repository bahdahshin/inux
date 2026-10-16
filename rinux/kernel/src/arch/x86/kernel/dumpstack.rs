// SPDX-License-Identifier: GPL-2.0
//! Oopses: `die()`, register dumps and stack traces.
//!
//! A partial port of `arch/x86/kernel/dumpstack.c`, `dump_stack_print_info()`
//! from `lib/dump_stack.c`, and the oops begin/end markers of
//! `kernel/panic.c`.
//!
//! The call trace follows frame pointers (the kernel is built with
//! `-C force-frame-pointers`), like Linux's `CONFIG_UNWINDER_FRAME_POINTER`.
//! There is no kallsyms yet, so addresses print as numbers, which is also
//! what Linux prints without `CONFIG_KALLSYMS`.
//!
//! Copyright (C) 1991, 1992  Linus Torvalds
//! Copyright (C) 2000, 2001, 2002 Andi Kleen, SuSE Labs

use core::fmt::Write;
use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use crate::arch::x86::include::irqflags::{local_irq_restore, local_irq_save};
use crate::arch::x86::include::kdebug::DIE_OOPS;
use crate::arch::x86::include::ptrace::{PtRegs, user_mode};
use crate::arch::x86::kernel::cpu::common::exception_stacks;
use crate::arch::x86::kernel::process_64::{__show_regs, ShowRegsMode};
use crate::arch::x86::kernel::traps::SIGSEGV;
use crate::init::version::UTS_RELEASE;
use crate::kernel::exit::{CURRENT_COMM, CURRENT_PID, make_task_dead};
use crate::kernel::locking::SpinLock;
use crate::kernel::notifier::{NOTIFY_STOP, notify_die};
use crate::kernel::panic::panic;
use crate::kernel::printk::{FmtBuf, LOGLEVEL_DEFAULT, bust_spinlocks, console_verbose};
use crate::kernel::softirq::in_interrupt;

/// `panic_on_oops`: `CONFIG_PANIC_ON_OOPS` is off by default.
pub static PANIC_ON_OOPS: AtomicBool = AtomicBool::new(false);

static DIE_COUNTER: AtomicU32 = AtomicU32::new(0);
static DIE_NEST_COUNT: AtomicU32 = AtomicU32::new(0);
/// `TAINT_DIE`: an oops happened.
static TAINT_DIE: AtomicBool = AtomicBool::new(false);
/// The registers of the first oops, repeated at the end of each one.
static EXEC_SUMMARY_REGS: SpinLock<Option<PtRegs>> = SpinLock::new(None);

unsafe extern "C" {
    static _stext: u8;
    static _etext: u8;
    static boot_stack: u8;
    static boot_stack_end: u8;
}

/// `oops_begin()`: interrupts off, everything to the console, and no
/// lock may stop us printing.
pub fn oops_begin() -> u64 {
    let flags = local_irq_save();
    // die_lock: with one CPU only the nesting count matters.
    DIE_NEST_COUNT.fetch_add(1, Ordering::Relaxed);
    console_verbose();
    bust_spinlocks();
    flags
}

/// `oops_end()`: prints the end marker and the executive summary, then
/// kills the current task.
pub fn oops_end(flags: u64, _regs: &PtRegs, signr: i32) {
    TAINT_DIE.store(true, Ordering::Relaxed);
    DIE_NEST_COUNT.fetch_sub(1, Ordering::Relaxed);
    local_irq_restore(flags);
    // oops_exit() -> print_oops_end_marker()
    pr_warn!("---[ end trace {:016x} ]---", 0);

    // Executive summary in case the oops scrolled away.
    if let Some(summary) = *EXEC_SUMMARY_REGS.lock_irqsave() {
        __show_regs(&summary, ShowRegsMode::All);
    }

    if signr == 0 {
        return;
    }
    if in_interrupt() {
        panic(format_args!("Fatal exception in interrupt"));
    }
    if PANIC_ON_OOPS.load(Ordering::Relaxed) {
        panic(format_args!("Fatal exception"));
    }
    // rewind_stack_and_make_dead()
    make_task_dead(signr);
}

fn __die_header(str: &str, regs: &PtRegs, err: i64) {
    // Save the registers of the first oops for the summary.
    if DIE_COUNTER.load(Ordering::Relaxed) == 0 {
        *EXEC_SUMMARY_REGS.lock_irqsave() = Some(*regs);
    }
    let n = DIE_COUNTER.fetch_add(1, Ordering::Relaxed) + 1;
    // The SMP, DEBUG_PAGEALLOC, KASAN and PTI markers are for options
    // rinux doesn't have.
    printk!(
        LOGLEVEL_DEFAULT,
        "Oops: {}: {:04x} [#{}]",
        str,
        err & 0xffff,
        n
    );
}

fn __die_body(str: &'static str, regs: &mut PtRegs, err: i64) -> bool {
    show_regs(regs);
    notify_die(DIE_OOPS, str, regs, err, 0, SIGSEGV) == NOTIFY_STOP
}

/// `__die()`: prints the oops. Returns true if a notifier handled it.
pub fn __die(str: &'static str, regs: &mut PtRegs, err: i64) -> bool {
    __die_header(str, regs, err);
    __die_body(str, regs, err)
}

/// `die()`: oopses and kills the current task.
pub fn die(str: &'static str, regs: &mut PtRegs, err: i64) {
    let flags = oops_begin();
    let sig = if __die(str, regs, err) { 0 } else { SIGSEGV };
    oops_end(flags, regs, sig);
}

/// `die_addr()`: like [`die`], for faults with an address KASAN may want.
pub fn die_addr(str: &'static str, regs: &mut PtRegs, err: i64, _gp_addr: u64) {
    let flags = oops_begin();
    __die_header(str, regs, err);
    let sig = if __die_body(str, regs, err) {
        0
    } else {
        SIGSEGV
    };
    oops_end(flags, regs, sig);
}

/// `print_tainted()`.
fn print_tainted() -> &'static str {
    if TAINT_DIE.load(Ordering::Relaxed) {
        "Tainted: [D]=DIE"
    } else {
        "Not tainted"
    }
}

/// `dump_stack_print_info()`: who was running.
pub fn dump_stack_print_info() {
    printk!(
        LOGLEVEL_DEFAULT,
        "CPU: {} UID: {} PID: {} Comm: {} {} {} {} {} ",
        0,
        0,
        CURRENT_PID,
        CURRENT_COMM,
        print_tainted(),
        UTS_RELEASE,
        "#1",
        "(none)"
    );
}

/// `show_regs()`.
pub fn show_regs(regs: &PtRegs) {
    dump_stack_print_info();
    let mode = if user_mode(regs) {
        ShowRegsMode::User
    } else {
        ShowRegsMode::All
    };
    __show_regs(regs, mode);
    // When in-kernel, also print the stack at the time of the fault.
    if !user_mode(regs) {
        show_trace_log_lvl(regs);
    }
}

const PROLOGUE_SIZE: usize = 42;
const EPILOGUE_SIZE: usize = 21;
const OPCODE_BUFSIZE: usize = PROLOGUE_SIZE + 1 + EPILOGUE_SIZE;

/// `show_opcodes()`: the code bytes around RIP, the faulting one in `<>`.
///
/// Linux reads them with `copy_from_kernel_nofault()`. Without exception
/// fixups rinux can only safely read its own text.
pub fn show_opcodes(regs: &PtRegs) {
    let prologue = regs.ip.wrapping_sub(PROLOGUE_SIZE as u64);
    let text = (&raw const _stext as u64)..(&raw const _etext as u64);
    if !text.contains(&prologue) || !text.contains(&(prologue + OPCODE_BUFSIZE as u64 - 1)) {
        printk!(
            LOGLEVEL_DEFAULT,
            "Code: Unable to access opcode bytes at {:#x}.",
            prologue
        );
        return;
    }
    // SAFETY: the whole range lies inside the kernel's mapped text.
    let opcodes = unsafe { core::slice::from_raw_parts(prologue as *const u8, OPCODE_BUFSIZE) };
    let mut line = FmtBuf::<256>::new();
    for b in &opcodes[..PROLOGUE_SIZE] {
        let _ = write!(line, "{b:02x} ");
    }
    let _ = write!(line, "<{:02x}>", opcodes[PROLOGUE_SIZE]);
    for b in &opcodes[PROLOGUE_SIZE + 1..] {
        let _ = write!(line, " {b:02x}");
    }
    printk!(LOGLEVEL_DEFAULT, "Code: {}", line);
}

/// `show_iret_regs()` with the `ORIG_RAX` that `__show_regs()` appends.
pub fn show_iret_regs(regs: &PtRegs) {
    // show_ip()
    printk!(
        LOGLEVEL_DEFAULT,
        "RIP: {:04x}:{:#018x}",
        regs.cs as u16,
        regs.ip
    );
    show_opcodes(regs);
    if regs.orig_ax != u64::MAX {
        printk!(
            LOGLEVEL_DEFAULT,
            "RSP: {:04x}:{:016x} EFLAGS: {:08x} ORIG_RAX: {:016x}",
            regs.ss as u16,
            regs.sp,
            regs.flags,
            regs.orig_ax
        );
    } else {
        printk!(
            LOGLEVEL_DEFAULT,
            "RSP: {:04x}:{:016x} EFLAGS: {:08x}",
            regs.ss as u16,
            regs.sp,
            regs.flags
        );
    }
}

/// Which stack `addr` is on: (name, bottom, top).
fn stack_info(addr: u64) -> Option<(&'static str, u64, u64)> {
    let task = (
        &raw const boot_stack as u64,
        &raw const boot_stack_end as u64,
    );
    if (task.0..task.1).contains(&addr) {
        return Some(("TASK", task.0, task.1));
    }
    exception_stacks()
        .into_iter()
        .map(|(name, lo, hi)| (name, lo as u64, hi as u64))
        .find(|&(_, lo, hi)| (lo..hi).contains(&addr))
}

/// `show_trace_log_lvl()`: the call trace, by frame pointers.
fn show_trace_log_lvl(regs: &PtRegs) {
    printk!(LOGLEVEL_DEFAULT, "Call Trace:");
    let Some((name, lo, hi)) = stack_info(regs.sp) else {
        return;
    };
    printk!(LOGLEVEL_DEFAULT, " <{}>", name);
    let mut bp = regs.bp;
    for _ in 0..64 {
        if bp % 8 != 0 || bp < lo || bp + 16 > hi {
            break;
        }
        // SAFETY: bp is an aligned frame inside a known kernel stack.
        let (next, ret) = unsafe { ((bp as *const u64).read(), (bp as *const u64).add(1).read()) };
        if ret == 0 {
            break;
        }
        printk!(LOGLEVEL_DEFAULT, " {:#018x}", ret);
        if next <= bp {
            break;
        }
        bp = next;
    }
    printk!(LOGLEVEL_DEFAULT, " </{}>", name);
}

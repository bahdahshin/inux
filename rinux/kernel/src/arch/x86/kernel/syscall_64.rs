// SPDX-License-Identifier: GPL-2.0
//! The SYSCALL entry path.
//!
//! A simplified port of `entry_SYSCALL_64` (`arch/x86/entry/entry_64.S`),
//! `syscall_init()`/`idt_syscall_init()` (`arch/x86/kernel/cpu/common.c`)
//! and `do_syscall_64()` (`arch/x86/entry/syscall_64.c`).
//!
//! On `SYSCALL` the CPU saves the user RIP in RCX and RFLAGS in R11, loads
//! RIP from `MSR_LSTAR` and CS/SS from `MSR_STAR`, and masks RFLAGS with
//! `MSR_SYSCALL_MASK` (which clears IF, so we enter with interrupts off). It
//! does *not* switch the stack. The stub below saves the user RSP, loads
//! this task's kernel stack, builds a `pt_regs`, calls [`do_syscall_64`],
//! and returns to user mode with `iretq` (the simple path; Linux also has a
//! `sysret` fast path).
//!
//! Simplified from Linux's: no `swapgs`/per-CPU GS (one CPU, a global holds
//! the kernel stack), no page-table isolation, and no 32-bit compat entry.

use core::arch::global_asm;

use crate::arch::x86::include::msr_index::{MSR_EFER, MSR_LSTAR, MSR_STAR, MSR_SYSCALL_MASK};
use crate::arch::x86::include::processor::wrmsr;
use crate::arch::x86::include::ptrace::PtRegs;
use crate::arch::x86::include::segment::{__KERNEL_CS, __USER32_CS};
use crate::kernel::locking::RacyCell;

/// `EFER.SCE`: enable `SYSCALL`/`SYSRET`.
const EFER_SCE: u64 = 1;

/// Flags cleared on SYSCALL entry (`MSR_SYSCALL_MASK`): TF, IF, DF, IOPL,
/// NT, AC, as Linux sets.
const SYSCALL_MASK: u64 = 0x0004_7700;

/// This CPU's current kernel stack top, loaded by the SYSCALL stub. Updated
/// on every context switch (what `cpu_current_top_of_stack` is per-CPU).
pub static KERNEL_STACK_TOP: RacyCell<u64> = RacyCell::new(0);

/// Scratch for the user RSP across the stack switch (`tss.sp2` in Linux).
static USER_RSP_SCRATCH: RacyCell<u64> = RacyCell::new(0);

unsafe extern "C" {
    fn entry_syscall_64();
}

/// `syscall_init()`: program the SYSCALL MSRs and enable `EFER.SCE`.
pub fn syscall_init() {
    // SAFETY: these are the architectural SYSCALL MSRs on every x86-64 CPU.
    unsafe {
        // STAR[63:48] = __USER32_CS (SYSRET base), STAR[47:32] = __KERNEL_CS.
        let star = ((__USER32_CS as u64) << 48) | ((__KERNEL_CS as u64) << 32);
        wrmsr(MSR_STAR as u32, star);
        wrmsr(MSR_LSTAR as u32, entry_syscall_64 as *const () as u64);
        wrmsr(MSR_SYSCALL_MASK as u32, SYSCALL_MASK);
        let efer = crate::arch::x86::include::processor::rdmsr(MSR_EFER as u32);
        wrmsr(MSR_EFER as u32, efer | EFER_SCE);
    }
}

global_asm!(
    r#"
    .text
    .globl entry_syscall_64
    .balign 16
entry_syscall_64:
    // Save the user stack and switch to this task's kernel stack.
    movq %rsp, {user_rsp}(%rip)
    movq {kstack}(%rip), %rsp

    // Build a pt_regs matching the iretq frame plus the saved registers.
    pushq ${user_ss}            // ss
    pushq {user_rsp}(%rip)      // sp
    pushq %r11                  // flags (saved by SYSCALL)
    pushq ${user_cs}            // cs
    pushq %rcx                  // ip (saved by SYSCALL)
    pushq %rax                  // orig_ax (syscall number)
    // PUSH_REGS, matching struct PtRegs below orig_ax.
    pushq %rdi
    pushq %rsi
    pushq %rdx
    pushq %rcx
    pushq %rax
    pushq %r8
    pushq %r9
    pushq %r10
    pushq %r11
    pushq %rbx
    pushq %rbp
    pushq %r12
    pushq %r13
    pushq %r14
    pushq %r15

    movq %rsp, %rdi             // pt_regs*
    andq $-16, %rsp             // align for the C ABI
    movq %rdi, %rbx             // save (callee-saved) across the call
    call {dispatch}
    movq %rbx, %rsp

    // Restore and return to user mode.
    popq %r15
    popq %r14
    popq %r13
    popq %r12
    popq %rbp
    popq %rbx
    popq %r11
    popq %r10
    popq %r9
    popq %r8
    popq %rax
    popq %rcx
    popq %rdx
    popq %rsi
    popq %rdi
    addq $8, %rsp              // skip orig_ax
    iretq
"#,
    user_rsp = sym USER_RSP_SCRATCH,
    kstack = sym KERNEL_STACK_TOP,
    dispatch = sym do_syscall_64,
    user_ss = const crate::arch::x86::include::segment::__USER_DS,
    user_cs = const crate::arch::x86::include::segment::__USER_CS,
    options(att_syntax),
);

/// `do_syscall_64()`: dispatch by syscall number (in `orig_ax`) and store
/// the result in `ax`.
extern "C" fn do_syscall_64(regs: &mut PtRegs) {
    let nr = regs.orig_ax;
    let ret = crate::kernel::syscalls::syscall_dispatch(nr, regs);
    regs.ax = ret as u64;
    // On the way back to user space, deliver a pending signal if one is due.
    crate::arch::x86::kernel::signal::do_signal(regs);
}

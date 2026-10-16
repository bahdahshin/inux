// SPDX-License-Identifier: GPL-2.0
//! The context switch and new-thread setup.
//!
//! A partial port of `__switch_to_asm` and `ret_from_fork_asm`
//! (`arch/x86/entry/entry_64.S`), `copy_thread()` (`arch/x86/kernel/process.c`)
//! and `kthread_frame_init()` (`arch/x86/include/asm/switch_to.h`).
//!
//! `__switch_to_asm` saves the callee-saved registers of `prev`, swaps the
//! kernel stack pointer, and restores `next`'s callee-saved registers. A
//! brand-new thread's stack is laid out so that the first switch "returns"
//! into [`kthread_trampoline`], which calls the thread function.
//!
//! Simplified from Linux's: kernel threads only, so no FPU, FS/GS base, TLS
//! or segment switching, and no `pt_regs`/user-return frame -- the full
//! `__switch_to` comes with user space.

use core::arch::global_asm;

use crate::include::linux::sched::{THREAD_SIZE, TaskStruct};
use crate::mm::__GFP_ZERO;
use crate::mm::page_alloc::{alloc_pages, free_pages};

unsafe extern "C" {
    /// `__switch_to_asm(prev_sp, next_sp)`: `prev_sp`/`next_sp` point at the
    /// `thread.sp` field of the previous and next task.
    pub fn switch_to_asm(prev_sp: *mut u64, next_sp: *mut u64);
    fn kthread_trampoline();
}

global_asm!(
    r#"
    .text
    .globl switch_to_asm
    .balign 16
switch_to_asm:
    // Save prev's callee-saved registers (order matches the new-thread
    // frame built in copy_thread).
    pushq %rbp
    pushq %rbx
    pushq %r12
    pushq %r13
    pushq %r14
    pushq %r15
    // Switch stacks: *prev_sp = rsp; rsp = *next_sp.
    movq %rsp, (%rdi)
    movq (%rsi), %rsp
    // Restore next's callee-saved registers.
    popq %r15
    popq %r14
    popq %r13
    popq %r12
    popq %rbx
    popq %rbp
    ret

    // A new kernel thread starts here, with fn in r12 and arg in r15, as
    // copy_thread arranged. Mirrors ret_from_fork_asm.
    .globl kthread_trampoline
    .balign 16
kthread_trampoline:
    movq %r12, %rdi             // fn
    movq %r15, %rsi             // arg
    xorq %rbp, %rbp             // end of stack for the unwinder
    call {bootstrap}
    ud2                         // kthread_bootstrap never returns
"#,
    bootstrap = sym kthread_bootstrap,
    options(att_syntax),
);

/// The C-ABI entry a new kernel thread reaches through the trampoline.
///
/// Mirrors `ret_from_fork()`: finish the switch, then run the thread
/// function and exit.
unsafe extern "C" fn kthread_bootstrap(func: u64, arg: u64) -> ! {
    use crate::arch::x86::include::irqflags::local_irq_enable;
    use crate::kernel::sched::core::{do_exit, schedule_tail};
    schedule_tail();
    // A new thread starts fresh: enable interrupts, as ret_from_fork returns
    // to a state with IF set. The switch into us came with interrupts off.
    local_irq_enable();
    // SAFETY: copy_thread stored a valid `extern "C" fn(u64) -> i32` in r12.
    let func: extern "C" fn(u64) -> i32 = unsafe { core::mem::transmute(func as *const ()) };
    let ret = func(arg);
    do_exit(ret);
}

/// `copy_thread()` for a kernel thread: allocate a stack and build the
/// initial switch frame so the first switch into `task` lands in the
/// trampoline with `func`/`arg` in the agreed registers.
///
/// Returns false if the stack could not be allocated.
pub fn copy_thread(task: &mut TaskStruct, func: extern "C" fn(u64) -> i32, arg: u64) -> bool {
    let order = crate::include::linux::sched::THREAD_SIZE_ORDER;
    let stack = alloc_pages(__GFP_ZERO, order);
    if stack == 0 {
        return false;
    }
    task.stack = stack;
    task.stack_order = order;
    task.kstack_top = stack + THREAD_SIZE as u64;

    // Build the frame popped by switch_to_asm, from the top down:
    // ret_addr, rbp, rbx, r12(func), r13, r14, r15(arg).
    let top = stack + THREAD_SIZE as u64;
    let frame = [
        arg,                                    // r15
        0,                                      // r14
        0,                                      // r13
        func as usize as u64,                   // r12
        0,                                      // rbx
        0,                                      // rbp
        kthread_trampoline as *const () as u64, // ret_addr
    ];
    let sp = top - (frame.len() * 8) as u64;
    // SAFETY: sp..top is inside the freshly allocated, identity-mapped stack.
    unsafe {
        for (i, &word) in frame.iter().enumerate() {
            (sp as *mut u64).add(i).write(word);
        }
    }
    task.thread.sp = sp;
    true
}

/// Frees a dead task's kernel stack.
///
/// # Safety
///
/// `task` must not be running and its stack must be unused.
pub unsafe fn free_thread_stack(task: &TaskStruct) {
    if task.stack != 0 {
        free_pages(task.stack, task.stack_order);
    }
}

/// `start_thread()`: enter user mode at `entry` with stack `user_sp`,
/// running on the user page tables `pgd`. Does not return.
///
/// Builds the `iretq` frame for ring 3 (SS/RSP/RFLAGS/CS/RIP) on the kernel
/// stack and returns into it, as `start_thread_common()` plus the exit-to-
/// user path do. Interrupts are enabled in user mode (RFLAGS.IF).
pub fn start_thread(pgd: u64, entry: u64, user_sp: u64) -> ! {
    use crate::arch::x86::include::irqflags::local_irq_disable;
    use crate::arch::x86::include::segment::{__USER_CS, __USER_DS};
    use crate::arch::x86::kernel::cpu::common::set_tss_sp0;
    use crate::arch::x86::kernel::syscall_64::KERNEL_STACK_TOP;
    use crate::arch::x86::mm::user::load_cr3;
    use crate::include::linux::sched::current;

    local_irq_disable();
    // SAFETY: current is the running task; use its kernel stack for traps
    // and syscalls taken from user mode.
    let kstack = unsafe { (*current()).kstack_top };
    set_tss_sp0(kstack);
    // SAFETY: single CPU; KERNEL_STACK_TOP is read only by the SYSCALL stub.
    unsafe { *KERNEL_STACK_TOP.get() = kstack };
    // SAFETY: pgd includes the kernel mappings, so the kernel stays mapped.
    unsafe { load_cr3(pgd) };

    // RFLAGS: reserved bit 1 set, IF set (interrupts on in user mode).
    let rflags: u64 = 0x202;
    // SAFETY: the iretq frame is valid and transfers to ring 3 at `entry`.
    unsafe {
        core::arch::asm!(
            // Point DS/ES at the user data segment.
            "movl {uds:e}, %eax",
            "movl %eax, %ds",
            "movl %eax, %es",
            // Build the iretq frame.
            "pushq {uds}",
            "pushq {usp}",
            "pushq {rflags}",
            "pushq {ucs}",
            "pushq {entry}",
            // Clear the general-purpose registers so no kernel data leaks.
            "xorl %eax, %eax",
            "xorl %ebx, %ebx",
            "xorl %ecx, %ecx",
            "xorl %edx, %edx",
            "xorl %esi, %esi",
            "xorl %edi, %edi",
            "xorl %ebp, %ebp",
            "xorl %r8d, %r8d",
            "xorl %r9d, %r9d",
            "xorl %r10d, %r10d",
            "xorl %r11d, %r11d",
            "xorl %r12d, %r12d",
            "xorl %r13d, %r13d",
            "xorl %r14d, %r14d",
            "xorl %r15d, %r15d",
            "iretq",
            uds = in(reg) __USER_DS as u64,
            usp = in(reg) user_sp,
            rflags = in(reg) rflags,
            ucs = in(reg) __USER_CS as u64,
            entry = in(reg) entry,
            options(att_syntax, noreturn),
        )
    }
}

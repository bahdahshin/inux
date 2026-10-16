// SPDX-License-Identifier: GPL-2.0-only
//! Exception and interrupt entry stubs.
//!
//! Ported from the `idtentry` macros of `arch/x86/entry/entry_64.S` and the
//! `irq_entries_start`/`spurious_entries_start` tables of
//! `arch/x86/include/asm/idtentry.h`.
//!
//! Each stub saves the registers as a `struct pt_regs` and calls its Rust
//! handler with a pointer to them (and the error code, for exceptions that
//! have one), then restores them and returns with IRETQ. There is no user
//! mode yet, so the GS swap, CR3 switch and user-return work of Linux's
//! `error_entry`/`error_return` are not needed.
//!
//! One deliberate difference: the kernel is built with 8-byte stack
//! alignment in C, but Rust code expects 16, so the stubs align the stack
//! before calling the handler and restore it afterwards.

use core::arch::global_asm;

use crate::arch::x86::include::irq_vectors::{
    FIRST_EXTERNAL_VECTOR, FIRST_SYSTEM_VECTOR, NR_VECTORS,
};
use crate::arch::x86::include::ptrace::ORIG_RAX;
use crate::arch::x86::include::trapnr::*;
use crate::arch::x86::kernel::irq::{common_interrupt, spurious_interrupt};
use crate::arch::x86::kernel::traps::*;
use crate::arch::x86::mm::fault::exc_page_fault;

/// `IDT_ALIGN`: size of each `irq_entries_start` stub (no IBT).
pub const IDT_ALIGN: usize = 8;

unsafe extern "C" {
    pub fn asm_exc_divide_error();
    pub fn asm_exc_debug();
    pub fn asm_exc_nmi();
    pub fn asm_exc_int3();
    pub fn asm_exc_overflow();
    pub fn asm_exc_bounds();
    pub fn asm_exc_invalid_op();
    pub fn asm_exc_device_not_available();
    pub fn asm_exc_double_fault();
    pub fn asm_exc_coproc_segment_overrun();
    pub fn asm_exc_invalid_tss();
    pub fn asm_exc_segment_not_present();
    pub fn asm_exc_stack_segment();
    pub fn asm_exc_general_protection();
    pub fn asm_exc_page_fault();
    pub fn asm_exc_spurious_interrupt_bug();
    pub fn asm_exc_coprocessor_error();
    pub fn asm_exc_alignment_check();
    pub fn asm_exc_machine_check();
    pub fn asm_exc_simd_coprocessor_error();
    pub fn asm_common_interrupt();
    pub fn asm_spurious_interrupt();
    /// `IDT_ALIGN`-byte stubs for vectors `FIRST_EXTERNAL_VECTOR` to
    /// `FIRST_SYSTEM_VECTOR - 1`.
    pub fn irq_entries_start();
    /// Stubs for the system vectors, which have no handlers yet.
    pub fn spurious_entries_start();
}

global_asm!(
    r#"
    .macro PUSH_REGS
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
    .endm

    .macro POP_REGS
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
    .endm

    /*
     * idtentry - an exception or interrupt entry stub.
     * vector:          vector number, or -1 for interrupt stubs
     * asmsym:          the stub's symbol
     * cfunc:           the handler: cfunc(regs) or cfunc(regs, error_code)
     * has_error_code:  1 if the CPU pushed an error code (or the stub
     *                  pushed the vector in its place)
     */
    .macro idtentry vector, asmsym, cfunc, has_error_code
    .globl \asmsym
    .balign 16
\asmsym:
    cld
    .if \has_error_code == 0
    pushq $-1                       /* ORIG_RAX: no syscall to restart */
    .endif

    .if \vector == {trap_bp}
    /*
     * If coming from kernel space, create a 6-word gap to allow the int3
     * handler to emulate a call instruction.
     */
    testb $3, 16(%rsp)              /* CS - ORIG_RAX */
    jnz 1f
    .rept 6
    pushq 5*8(%rsp)
    .endr
1:
    .endif

    PUSH_REGS
    movq %rsp, %rdi                 /* pt_regs pointer into 1st argument */
    .if \has_error_code == 1
    movq {orig_rax}(%rsp), %rsi     /* get error code into 2nd argument */
    movq $-1, {orig_rax}(%rsp)      /* no syscall to restart */
    .endif

    movq %rsp, %rbx                 /* callee-saved: undo the alignment */
    andq $-16, %rsp
    call \cfunc
    movq %rbx, %rsp

    POP_REGS
    addq $8, %rsp                   /* skip orig_ax */
    iretq
    .endm

    .pushsection .text.entry, "ax"

    idtentry {trap_de}, asm_exc_divide_error, {exc_divide_error}, 0
    idtentry {trap_db}, asm_exc_debug, {exc_debug}, 0
    idtentry {trap_nmi}, asm_exc_nmi, {exc_nmi}, 0
    idtentry {trap_bp}, asm_exc_int3, {exc_int3}, 0
    idtentry {trap_of}, asm_exc_overflow, {exc_overflow}, 0
    idtentry {trap_br}, asm_exc_bounds, {exc_bounds}, 0
    idtentry {trap_ud}, asm_exc_invalid_op, {exc_invalid_op}, 0
    idtentry {trap_nm}, asm_exc_device_not_available, {exc_device_not_available}, 0
    idtentry {trap_df}, asm_exc_double_fault, {exc_double_fault}, 1
    idtentry {trap_old_mf}, asm_exc_coproc_segment_overrun, {exc_coproc_segment_overrun}, 0
    idtentry {trap_ts}, asm_exc_invalid_tss, {exc_invalid_tss}, 1
    idtentry {trap_np}, asm_exc_segment_not_present, {exc_segment_not_present}, 1
    idtentry {trap_ss}, asm_exc_stack_segment, {exc_stack_segment}, 1
    idtentry {trap_gp}, asm_exc_general_protection, {exc_general_protection}, 1
    idtentry {trap_pf}, asm_exc_page_fault, {exc_page_fault}, 1
    idtentry {trap_spurious}, asm_exc_spurious_interrupt_bug, {exc_spurious_interrupt_bug}, 0
    idtentry {trap_mf}, asm_exc_coprocessor_error, {exc_coprocessor_error}, 0
    idtentry {trap_ac}, asm_exc_alignment_check, {exc_alignment_check}, 1
    idtentry {trap_mc}, asm_exc_machine_check, {exc_machine_check}, 0
    idtentry {trap_xf}, asm_exc_simd_coprocessor_error, {exc_simd_coprocessor_error}, 0

    /* The stubs below push the vector where the error code goes. */
    idtentry -1, asm_common_interrupt, {common_interrupt}, 1
    idtentry -1, asm_spurious_interrupt, {spurious_interrupt}, 1

    /*
     * Each stub pushes its vector (sign-extended from 8 bits; handlers use
     * the low byte) and jumps to the common entry: 2 + 5 + 1 padding =
     * IDT_ALIGN bytes. The jump is encoded by hand so its size is fixed.
     */
    .balign {idt_align}
    .globl irq_entries_start
irq_entries_start:
    vector = {first_external}
    .rept {first_system} - {first_external}
    .byte 0x6a, vector              /* pushq $vector */
    .byte 0xe9                      /* jmp asm_common_interrupt */
    .long asm_common_interrupt - . - 4
    .byte 0xcc
    vector = vector + 1
    .endr

    .balign {idt_align}
    .globl spurious_entries_start
spurious_entries_start:
    vector = {first_system}
    .rept {nr_vectors} - {first_system}
    .byte 0x6a, vector
    .byte 0xe9
    .long asm_spurious_interrupt - . - 4
    .byte 0xcc
    vector = vector + 1
    .endr

    .popsection
"#,
    orig_rax = const ORIG_RAX,
    idt_align = const IDT_ALIGN,
    first_external = const FIRST_EXTERNAL_VECTOR,
    first_system = const FIRST_SYSTEM_VECTOR,
    nr_vectors = const NR_VECTORS,
    trap_de = const X86_TRAP_DE,
    trap_db = const X86_TRAP_DB,
    trap_nmi = const X86_TRAP_NMI,
    trap_bp = const X86_TRAP_BP,
    trap_of = const X86_TRAP_OF,
    trap_br = const X86_TRAP_BR,
    trap_ud = const X86_TRAP_UD,
    trap_nm = const X86_TRAP_NM,
    trap_df = const X86_TRAP_DF,
    trap_old_mf = const X86_TRAP_OLD_MF,
    trap_ts = const X86_TRAP_TS,
    trap_np = const X86_TRAP_NP,
    trap_ss = const X86_TRAP_SS,
    trap_gp = const X86_TRAP_GP,
    trap_pf = const X86_TRAP_PF,
    trap_spurious = const X86_TRAP_SPURIOUS,
    trap_mf = const X86_TRAP_MF,
    trap_ac = const X86_TRAP_AC,
    trap_mc = const X86_TRAP_MC,
    trap_xf = const X86_TRAP_XF,
    exc_divide_error = sym exc_divide_error,
    exc_debug = sym exc_debug,
    exc_nmi = sym exc_nmi,
    exc_int3 = sym exc_int3,
    exc_overflow = sym exc_overflow,
    exc_bounds = sym exc_bounds,
    exc_invalid_op = sym exc_invalid_op,
    exc_device_not_available = sym exc_device_not_available,
    exc_double_fault = sym exc_double_fault,
    exc_coproc_segment_overrun = sym exc_coproc_segment_overrun,
    exc_invalid_tss = sym exc_invalid_tss,
    exc_segment_not_present = sym exc_segment_not_present,
    exc_stack_segment = sym exc_stack_segment,
    exc_general_protection = sym exc_general_protection,
    exc_page_fault = sym exc_page_fault,
    exc_spurious_interrupt_bug = sym exc_spurious_interrupt_bug,
    exc_coprocessor_error = sym exc_coprocessor_error,
    exc_alignment_check = sym exc_alignment_check,
    exc_machine_check = sym exc_machine_check,
    exc_simd_coprocessor_error = sym exc_simd_coprocessor_error,
    common_interrupt = sym common_interrupt,
    spurious_interrupt = sym spurious_interrupt,
    options(att_syntax),
);

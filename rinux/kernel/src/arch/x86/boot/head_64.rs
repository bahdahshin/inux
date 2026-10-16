// SPDX-License-Identifier: GPL-2.0
//! The 32-bit entry point and the switch to 64-bit mode.
//!
//! GRUB's multiboot2 loader enters `startup_32` in 32-bit protected mode
//! with paging off, EAX = the multiboot2 magic and EBX = the physical
//! address of the boot information. From there this follows `startup_32`
//! in `arch/x86/boot/compressed/head_64.S`:
//!
//! 1. load a GDT with 32- and 64-bit code segments,
//! 2. check the CPU with `verify_cpu` (ported from
//!    `arch/x86/kernel/verify_cpu.S`, included below),
//! 3. enable PAE and build page tables identity-mapping the first 4 GiB
//!    with 2 MiB pages,
//! 4. set EFER.LME, enable paging, and far-return into 64-bit code.
//!
//! Linux's `startup_32` also computes its load address, relocates and
//! decompresses the kernel, and handles EFI and AMD SEV. rinux is loaded
//! uncompressed at its link address, so none of that is needed. The 64-bit
//! side then calls `x86_64_start_kernel()`, as `arch/x86/kernel/head_64.S`
//! does.

use core::arch::global_asm;

use crate::arch::x86::include::msr_index::{
    _EFER_LME, _EFER_NX, MSR_EFER, MSR_IA32_MISC_ENABLE, MSR_K7_HWCR,
};
use crate::arch::x86::include::processor_flags::{CR0_STATE, X86_CR4_PAE};
use crate::arch::x86::kernel::head64::x86_64_start_kernel;

/// Value in EAX when a multiboot2 loader enters the kernel.
pub const MULTIBOOT2_BOOTLOADER_MAGIC: u32 = 0x36d7_6289;
const MULTIBOOT2_HEADER_MAGIC: u32 = 0xe852_50d6;
const MULTIBOOT_ARCHITECTURE_I386: u32 = 0;

// Segment selectors, from arch/x86/include/asm/segment.h. The boot GDT
// below has these descriptors in this order.
const GDT_ENTRY_KERNEL32_CS: u32 = 1;
const GDT_ENTRY_KERNEL_CS: u32 = 2;
const GDT_ENTRY_BOOT_DS: u32 = 3;
const GDT_ENTRY_BOOT_TSS: u32 = 4;
const __KERNEL32_CS: u32 = GDT_ENTRY_KERNEL32_CS * 8;
const __KERNEL_CS: u32 = GDT_ENTRY_KERNEL_CS * 8;
const __BOOT_DS: u32 = GDT_ENTRY_BOOT_DS * 8;
const __BOOT_TSS: u32 = GDT_ENTRY_BOOT_TSS * 8;

/// One PML4, one PDPT and four page directories: 4 GiB in 2 MiB pages.
const BOOT_INIT_PGT_SIZE: u32 = 6 * 4096;
/// `THREAD_SIZE` on x86-64.
const BOOT_STACK_SIZE: u32 = 16 * 1024;

// CPUID feature bits from arch/x86/include/asm/cpufeatures.h.
const X86_FEATURE_FPU: u32 = 0;
const X86_FEATURE_PSE: u32 = 3;
const X86_FEATURE_MSR: u32 = 5;
const X86_FEATURE_PAE: u32 = 6;
const X86_FEATURE_CX8: u32 = 8;
const X86_FEATURE_PGE: u32 = 13;
const X86_FEATURE_CMOV: u32 = 15;
const X86_FEATURE_FXSR: u32 = 24;
const X86_FEATURE_XMM: u32 = 25;
const X86_FEATURE_XMM2: u32 = 26;
const X86_FEATURE_LM: u32 = 29;

/// Features of CPUID leaf 1 EDX that `Kconfig.cpufeatures` requires on
/// x86-64: the generated `REQUIRED_MASK0`.
const REQUIRED_MASK0: u32 = (1 << X86_FEATURE_FPU)
    | (1 << X86_FEATURE_PSE)
    | (1 << X86_FEATURE_MSR)
    | (1 << X86_FEATURE_PAE)
    | (1 << X86_FEATURE_CX8)
    | (1 << X86_FEATURE_PGE)
    | (1 << X86_FEATURE_CMOV)
    | (1 << X86_FEATURE_FXSR)
    | (1 << X86_FEATURE_XMM)
    | (1 << X86_FEATURE_XMM2);
/// Required features of CPUID leaf 0x80000001 EDX: long mode.
const REQUIRED_MASK1: u32 = 1 << X86_FEATURE_LM;
const SSE_MASK: u32 = REQUIRED_MASK0 & ((1 << X86_FEATURE_XMM) | (1 << X86_FEATURE_XMM2));

global_asm!(
    r#"
    /* The multiboot2 header (Multiboot2 specification, section 3.1). */
    .section .multiboot2, "a"
    .balign 8
multiboot2_header:
    .long {mb2_magic}
    .long {mb2_arch}
    .long multiboot2_header_end - multiboot2_header
    .long 0x100000000 - ({mb2_magic} + {mb2_arch} + (multiboot2_header_end - multiboot2_header))
    /* End tag. */
    .balign 8
    .short 0
    .short 0
    .long 8
multiboot2_header_end:

    .section .head.text, "ax"
    .code32
    .globl startup_32
startup_32:
    cld
    cli

    /* Keep the boot protocol registers; verify_cpu clobbers them. */
    movl %eax, multiboot2_magic
    movl %ebx, multiboot2_info

    /* Load a new GDT with the 64-bit segments using a 32-bit descriptor. */
    lgdt gdt

    /* Load segment registers with our descriptors. */
    movl ${boot_ds}, %eax
    movl %eax, %ds
    movl %eax, %es
    movl %eax, %fs
    movl %eax, %gs
    movl %eax, %ss

    /* Set up a stack and load CS from the new GDT. */
    movl $boot_stack_end, %esp
    pushl ${kernel32_cs}
    pushl $1f
    lretl
1:
    /* Make sure the CPU supports long mode. */
    call verify_cpu
    testl %eax, %eax
    jnz .Lno_longmode

    /* Enable PAE mode. */
    movl %cr4, %eax
    orl ${cr4_pae}, %eax
    movl %eax, %cr4

    /* Build the early 4G boot page table. First zero it. */
    movl $pgtable, %edi
    xorl %eax, %eax
    movl ${pgt_size} / 4, %ecx
    rep stosl

    /* Level 4. */
    movl $pgtable, %edi
    leal 0x1007(%edi), %eax
    movl %eax, 0(%edi)

    /* Level 3. */
    movl $pgtable + 0x1000, %edi
    leal 0x1007(%edi), %eax
    movl $4, %ecx
1:  movl %eax, 0x00(%edi)
    addl $0x00001000, %eax
    addl $8, %edi
    decl %ecx
    jnz 1b

    /* Level 2: 2048 2 MiB pages, present + writable + large + global. */
    movl $pgtable + 0x2000, %edi
    movl $0x00000183, %eax
    movl $2048, %ecx
1:  movl %eax, 0(%edi)
    addl $0x00200000, %eax
    addl $8, %edi
    decl %ecx
    jnz 1b

    /* Enable the boot page tables. */
    movl $pgtable, %eax
    movl %eax, %cr3

    /* Enable long mode in EFER (Extended Feature Enable Register). */
    movl ${msr_efer}, %ecx
    rdmsr
    btsl ${efer_lme}, %eax
    btsl ${efer_nx}, %eax               /* Enable no-execute (EFER.NXE) */
    wrmsr

    /* After the GDT is loaded. */
    xorl %eax, %eax
    lldt %ax
    movl ${boot_tss}, %eax
    ltr %ax

    /*
     * Set up the jump to 64-bit mode. Turning on paging puts us in long
     * mode but in 32-bit compatibility mode (CS.L = 0, CS.D = 1); the far
     * return through __KERNEL_CS, which has CS.L = 1, enters 64-bit mode.
     */
    pushl ${kernel_cs}
    pushl $startup_64

    /* Enter paged protected mode, activating long mode. */
    movl ${cr0_state}, %eax
    movl %eax, %cr0

    /* Jump from 32-bit compatibility mode into 64-bit mode. */
    lret

.Lno_longmode:
    /* As arch/x86/boot/cpu.c would say it, on the VGA text screen. */
    movl $no_longmode_msg, %esi
    movl $0xb8000, %edi
1:  lodsb
    testb %al, %al
    jz 2f
    movb $0x07, %ah
    stosw
    jmp 1b
2:  hlt
    jmp 2b

/*
 * verify_cpu: check for long mode and SSE, ported from
 * arch/x86/kernel/verify_cpu.S. Returns 0 in %eax on success, 1 on
 * failure. On Intel, clears the XD_DISABLE flag as a side effect.
 */
verify_cpu:
    pushfl                          /* Save caller passed flags */
    pushl $0                        /* Kill any dangerous flags */
    popfl

    pushfl                          /* Standard way to check for cpuid */
    popl %eax
    movl %eax, %ebx
    xorl $0x200000, %eax
    pushl %eax
    popfl
    pushfl
    popl %eax
    cmpl %eax, %ebx
    jz .Lverify_cpu_no_longmode     /* CPU has no cpuid */

    movl $0x0, %eax                 /* See if cpuid 1 is implemented */
    cpuid
    cmpl $0x1, %eax
    jb .Lverify_cpu_no_longmode     /* No cpuid 1 */

    xor %di, %di
    cmpl $0x68747541, %ebx          /* AuthenticAMD */
    jnz .Lverify_cpu_noamd
    cmpl $0x69746e65, %edx
    jnz .Lverify_cpu_noamd
    cmpl $0x444d4163, %ecx
    jnz .Lverify_cpu_noamd
    mov $1, %di                     /* CPU is from AMD */
    jmp .Lverify_cpu_check

.Lverify_cpu_noamd:
    cmpl $0x756e6547, %ebx          /* GenuineIntel? */
    jnz .Lverify_cpu_check
    cmpl $0x49656e69, %edx
    jnz .Lverify_cpu_check
    cmpl $0x6c65746e, %ecx
    jnz .Lverify_cpu_check

    /* Only touch IA32_MISC_ENABLE on family > 6 || (family 6, model >= 0xd). */
    movl $0x1, %eax                 /* Check CPU family and model */
    cpuid
    movl %eax, %ecx

    andl $0x0ff00f00, %eax          /* Mask family and extended family */
    shrl $8, %eax
    cmpl $6, %eax
    ja .Lverify_cpu_clear_xd        /* Family > 6, ok */
    jb .Lverify_cpu_check           /* Family < 6, skip */

    andl $0x000f00f0, %ecx          /* Mask model and extended model */
    shrl $4, %ecx
    cmpl $0xd, %ecx
    jb .Lverify_cpu_check           /* Family 6, model < 0xd, skip */

.Lverify_cpu_clear_xd:
    movl ${msr_misc_enable}, %ecx
    rdmsr
    btrl $2, %edx                   /* Clear MSR_IA32_MISC_ENABLE_XD_DISABLE */
    jnc .Lverify_cpu_check          /* Only write the MSR if the bit changed */
    wrmsr

.Lverify_cpu_check:
    movl $0x1, %eax                 /* Does the CPU have what it takes? */
    cpuid
    andl ${required_mask0}, %edx
    xorl ${required_mask0}, %edx
    jnz .Lverify_cpu_no_longmode

    movl $0x80000000, %eax          /* See if extended cpuid is implemented */
    cpuid
    cmpl $0x80000001, %eax
    jb .Lverify_cpu_no_longmode     /* No extended cpuid */

    movl $0x80000001, %eax          /* Does the CPU have what it takes? */
    cpuid
    andl ${required_mask1}, %edx
    xorl ${required_mask1}, %edx
    jnz .Lverify_cpu_no_longmode

.Lverify_cpu_sse_test:
    movl $1, %eax
    cpuid
    andl ${sse_mask}, %edx
    cmpl ${sse_mask}, %edx
    je .Lverify_cpu_sse_ok
    test %di, %di
    jz .Lverify_cpu_no_longmode     /* Only try to force SSE on AMD */
    movl ${msr_k7_hwcr}, %ecx
    rdmsr
    btr $15, %eax                   /* Enable SSE */
    wrmsr
    xor %di, %di                    /* Don't loop */
    jmp .Lverify_cpu_sse_test       /* Try again */

.Lverify_cpu_no_longmode:
    popfl                           /* Restore caller passed flags */
    movl $1, %eax
    ret
.Lverify_cpu_sse_ok:
    popfl                           /* Restore caller passed flags */
    xorl %eax, %eax
    ret

    .code64
    .balign 16
startup_64:
    /* Data segments are ignored in 64-bit mode; load null selectors. */
    xorl %eax, %eax
    movl %eax, %ds
    movl %eax, %es
    movl %eax, %ss
    movl %eax, %fs
    movl %eax, %gs

    leaq boot_stack_end(%rip), %rsp
    pushq $0
    popfq
    xorl %ebp, %ebp

    /* x86_64_start_kernel(magic, boot information address) */
    movl multiboot2_magic(%rip), %edi
    movl multiboot2_info(%rip), %esi
    call {start_kernel}
1:  cli
    hlt
    jmp 1b

    .section .data, "aw"
    .balign 8
gdt:
    .word gdt_end - gdt - 1
    .long gdt
    .word 0
    .quad 0x00cf9a000000ffff        /* __KERNEL32_CS */
    .quad 0x00af9a000000ffff        /* __KERNEL_CS */
    .quad 0x00cf92000000ffff        /* __KERNEL_DS */
    .quad 0x0080890000000000        /* TS descriptor */
    .quad 0x0000000000000000        /* TS continued */
gdt_end:

    .balign 4
multiboot2_magic:
    .long 0
multiboot2_info:
    .long 0

    .section .rodata, "a"
no_longmode_msg:
    .asciz "This kernel requires an x86-64 CPU."

    .section .bss, "aw", @nobits
    .balign 4096
pgtable:
    .skip {pgt_size}
    .balign 16
    .globl boot_stack, boot_stack_end
boot_stack:
    .skip {stack_size}
boot_stack_end:

    .text
"#,
    mb2_magic = const MULTIBOOT2_HEADER_MAGIC,
    mb2_arch = const MULTIBOOT_ARCHITECTURE_I386,
    boot_ds = const __BOOT_DS,
    kernel32_cs = const __KERNEL32_CS,
    kernel_cs = const __KERNEL_CS,
    boot_tss = const __BOOT_TSS,
    cr4_pae = const X86_CR4_PAE,
    cr0_state = const CR0_STATE,
    msr_efer = const MSR_EFER,
    efer_lme = const _EFER_LME,
    efer_nx = const _EFER_NX,
    msr_misc_enable = const MSR_IA32_MISC_ENABLE,
    msr_k7_hwcr = const MSR_K7_HWCR,
    required_mask0 = const REQUIRED_MASK0,
    required_mask1 = const REQUIRED_MASK1,
    sse_mask = const SSE_MASK,
    pgt_size = const BOOT_INIT_PGT_SIZE,
    stack_size = const BOOT_STACK_SIZE,
    start_kernel = sym x86_64_start_kernel,
    options(att_syntax),
);

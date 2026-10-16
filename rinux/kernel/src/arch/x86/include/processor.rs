// SPDX-License-Identifier: GPL-2.0
//! CPU access: CPUID, control registers, MSRs, the TSC and the hardware TSS.
//!
//! Ported from parts of `arch/x86/include/asm/processor.h`,
//! `arch/x86/include/asm/special_insns.h`, `arch/x86/include/asm/msr.h` and
//! `arch/x86/include/asm/tsc.h`.

use core::arch::asm;

/// `native_cpuid()`: returns `(eax, ebx, ecx, edx)` for `leaf`/`subleaf`.
#[inline]
pub fn cpuid_count(leaf: u32, subleaf: u32) -> (u32, u32, u32, u32) {
    let r = core::arch::x86_64::__cpuid_count(leaf, subleaf);
    (r.eax, r.ebx, r.ecx, r.edx)
}

/// `cpuid()`: CPUID with subleaf 0.
#[inline]
pub fn cpuid(leaf: u32) -> (u32, u32, u32, u32) {
    cpuid_count(leaf, 0)
}

/// `cpuid_eax()`.
#[inline]
pub fn cpuid_eax(leaf: u32) -> u32 {
    cpuid(leaf).0
}

/// `cpuid_edx()`.
#[inline]
pub fn cpuid_edx(leaf: u32) -> u32 {
    cpuid(leaf).3
}

/// `read_cr0()`.
#[inline]
pub fn read_cr0() -> u64 {
    let v: u64;
    // SAFETY: reading CR0 has no side effects.
    unsafe {
        asm!("mov %cr0, {}", out(reg) v, options(att_syntax, nomem, nostack, preserves_flags))
    }
    v
}

/// `read_cr2()`: the faulting address of the last page fault.
#[inline]
pub fn read_cr2() -> u64 {
    let v: u64;
    // SAFETY: reading CR2 has no side effects.
    unsafe {
        asm!("mov %cr2, {}", out(reg) v, options(att_syntax, nomem, nostack, preserves_flags))
    }
    v
}

/// `__read_cr3()`.
#[inline]
pub fn read_cr3() -> u64 {
    let v: u64;
    // SAFETY: reading CR3 has no side effects.
    unsafe {
        asm!("mov %cr3, {}", out(reg) v, options(att_syntax, nomem, nostack, preserves_flags))
    }
    v
}

/// `__read_cr4()`.
#[inline]
pub fn read_cr4() -> u64 {
    let v: u64;
    // SAFETY: reading CR4 has no side effects.
    unsafe {
        asm!("mov %cr4, {}", out(reg) v, options(att_syntax, nomem, nostack, preserves_flags))
    }
    v
}

/// `rdmsrq()`: reads a model-specific register.
///
/// # Safety
///
/// The MSR must exist on this CPU, or the read faults with #GP.
#[inline]
pub unsafe fn rdmsr(msr: u32) -> u64 {
    let (lo, hi): (u32, u32);
    // SAFETY: the caller guarantees the MSR exists.
    unsafe {
        asm!("rdmsr", in("ecx") msr, out("eax") lo, out("edx") hi, options(nomem, nostack, preserves_flags))
    }
    (hi as u64) << 32 | lo as u64
}

/// `wrmsrq()`: writes a model-specific register.
///
/// # Safety
///
/// The MSR must exist, and the value must be one the kernel can live with.
#[inline]
pub unsafe fn wrmsr(msr: u32, value: u64) {
    // SAFETY: the caller guarantees the write is valid.
    unsafe {
        asm!("wrmsr", in("ecx") msr, in("eax") value as u32, in("edx") (value >> 32) as u32, options(nostack, preserves_flags))
    }
}

/// `rdtsc()`: the time-stamp counter.
#[inline]
pub fn rdtsc() -> u64 {
    // SAFETY: RDTSC has no side effects.
    unsafe { core::arch::x86_64::_rdtsc() }
}

/// `rdtsc_ordered()`: RDTSC that is not executed ahead of earlier loads.
#[inline]
pub fn rdtsc_ordered() -> u64 {
    let (lo, hi): (u32, u32);
    // SAFETY: RDTSCP-free ordering: LFENCE then RDTSC, as Linux does when
    // RDTSCP is not used.
    unsafe {
        asm!("lfence", "rdtsc", out("eax") lo, out("edx") hi, options(nomem, nostack, preserves_flags))
    }
    (hi as u64) << 32 | lo as u64
}

/// `get_cycles()`.
#[inline]
pub fn get_cycles() -> u64 {
    rdtsc()
}

/// `cpu_relax()`: `pause` in a spin loop.
#[inline]
pub fn cpu_relax() {
    core::hint::spin_loop();
}

/// `struct x86_hw_tss`: the 64-bit task-state segment.
#[repr(C, packed)]
#[derive(Clone, Copy, Default)]
pub struct X86HwTss {
    pub reserved1: u32,
    pub sp0: u64,
    pub sp1: u64,
    pub sp2: u64,
    pub reserved2: u64,
    pub ist: [u64; 7],
    pub reserved3: u32,
    pub reserved4: u32,
    pub reserved5: u16,
    pub io_bitmap_base: u16,
}

/// `__KERNEL_TSS_LIMIT` without the I/O bitmap (`CONFIG_X86_IOPL_IOPERM`).
pub const __KERNEL_TSS_LIMIT: u32 = core::mem::size_of::<X86HwTss>() as u32 - 1;
/// `IO_BITMAP_OFFSET_INVALID`: an I/O bitmap offset past the TSS limit, so
/// every port access from user space faults.
pub const IO_BITMAP_OFFSET_INVALID: u16 = (__KERNEL_TSS_LIMIT + 1) as u16;

const _: () = assert!(core::mem::size_of::<X86HwTss>() == 104);

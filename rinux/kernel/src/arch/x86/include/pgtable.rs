// SPDX-License-Identifier: GPL-2.0
//! Page table entry flags and the 4-level paging layout.
//!
//! Ported from `arch/x86/include/asm/pgtable_types.h` and
//! `arch/x86/include/asm/pgtable_64_types.h`. rinux uses 4-level paging
//! (no 5-level/LA57), which is what `startup_32` set up.

// Page table entry bit positions (`_PAGE_BIT_*`).
pub const _PAGE_BIT_PRESENT: u64 = 0;
pub const _PAGE_BIT_RW: u64 = 1;
pub const _PAGE_BIT_USER: u64 = 2;
pub const _PAGE_BIT_PWT: u64 = 3;
pub const _PAGE_BIT_PCD: u64 = 4;
pub const _PAGE_BIT_ACCESSED: u64 = 5;
pub const _PAGE_BIT_DIRTY: u64 = 6;
pub const _PAGE_BIT_PSE: u64 = 7;
pub const _PAGE_BIT_GLOBAL: u64 = 8;
pub const _PAGE_BIT_NX: u64 = 63;

pub const _PAGE_PRESENT: u64 = 1 << _PAGE_BIT_PRESENT;
pub const _PAGE_RW: u64 = 1 << _PAGE_BIT_RW;
pub const _PAGE_USER: u64 = 1 << _PAGE_BIT_USER;
pub const _PAGE_PWT: u64 = 1 << _PAGE_BIT_PWT;
pub const _PAGE_PCD: u64 = 1 << _PAGE_BIT_PCD;
pub const _PAGE_ACCESSED: u64 = 1 << _PAGE_BIT_ACCESSED;
pub const _PAGE_DIRTY: u64 = 1 << _PAGE_BIT_DIRTY;
/// Large page at the PMD/PUD level.
pub const _PAGE_PSE: u64 = 1 << _PAGE_BIT_PSE;
pub const _PAGE_GLOBAL: u64 = 1 << _PAGE_BIT_GLOBAL;
/// No-execute.
pub const _PAGE_NX: u64 = 1 << _PAGE_BIT_NX;

/// `__PAGE_KERNEL`: present, writable, accessed, dirty, global.
pub const PAGE_KERNEL: u64 =
    _PAGE_PRESENT | _PAGE_RW | _PAGE_ACCESSED | _PAGE_DIRTY | _PAGE_GLOBAL | _PAGE_NX;
/// `PAGE_KERNEL_EXEC`: like [`PAGE_KERNEL`] but executable.
pub const PAGE_KERNEL_EXEC: u64 =
    _PAGE_PRESENT | _PAGE_RW | _PAGE_ACCESSED | _PAGE_DIRTY | _PAGE_GLOBAL;
/// `PAGE_KERNEL_RO`: read-only kernel data.
pub const PAGE_KERNEL_RO: u64 =
    _PAGE_PRESENT | _PAGE_ACCESSED | _PAGE_DIRTY | _PAGE_GLOBAL | _PAGE_NX;

/// The physical-address bits of a page table entry.
pub const PTE_PFN_MASK: u64 = 0x000f_ffff_ffff_f000;
/// Flag bits of a page table entry.
pub const PTE_FLAGS_MASK: u64 = !PTE_PFN_MASK;

// 4-level paging shifts and fan-out.
pub const PGDIR_SHIFT: u32 = 39;
pub const PUD_SHIFT: u32 = 30;
pub const PMD_SHIFT: u32 = 21;
pub const PTRS_PER_PGD: usize = 512;
pub const PTRS_PER_PUD: usize = 512;
pub const PTRS_PER_PMD: usize = 512;
pub const PTRS_PER_PTE: usize = 512;

/// `PMD_SIZE`: bytes mapped by one PMD entry (2 MiB).
pub const PMD_SIZE: u64 = 1 << PMD_SHIFT;
/// `PUD_SIZE`: bytes mapped by one PUD entry (1 GiB).
pub const PUD_SIZE: u64 = 1 << PUD_SHIFT;

/// `pgd_index()`.
#[inline]
pub const fn pgd_index(addr: u64) -> usize {
    (addr >> PGDIR_SHIFT) as usize & (PTRS_PER_PGD - 1)
}

/// `pud_index()`.
#[inline]
pub const fn pud_index(addr: u64) -> usize {
    (addr >> PUD_SHIFT) as usize & (PTRS_PER_PUD - 1)
}

/// `pmd_index()`.
#[inline]
pub const fn pmd_index(addr: u64) -> usize {
    (addr >> PMD_SHIFT) as usize & (PTRS_PER_PMD - 1)
}

/// `pte_index()`.
#[inline]
pub const fn pte_index(addr: u64) -> usize {
    use super::page::PAGE_SHIFT;
    (addr >> PAGE_SHIFT) as usize & (PTRS_PER_PTE - 1)
}

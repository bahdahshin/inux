// SPDX-License-Identifier: GPL-2.0
//! Building the kernel's page tables.
//!
//! A partial port of `arch/x86/mm/init.c` and `init_64.c`. `startup_32`
//! left boot page tables that identity-map the first 4 GiB with 2 MiB
//! pages. `init_mem_mapping()` replaces them with kernel page tables that
//! map all of RAM as a direct map and, unlike the boot tables, leave page 0
//! unmapped so a NULL dereference faults. Page-table pages come from
//! memblock.
//!
//! rinux runs identity-mapped (`PAGE_OFFSET` = 0), so the direct map is the
//! identity map. Linux's split of the address space, `__va`/`__pa` offset,
//! KASLR and 2 MiB/1 GiB probing of usable page sizes come when the kernel
//! moves to the top of the address space.

use core::sync::atomic::{AtomicU64, Ordering};

use crate::arch::x86::include::page::{PAGE_SIZE, page_align};
use crate::arch::x86::include::pgtable::*;
use crate::arch::x86::include::processor::read_cr4;
use crate::mm::memblock::{alloc as memblock_alloc, max_pfn, memblock_reserve};

/// The physical address of the kernel PGD, once built.
static KERNEL_PGD: AtomicU64 = AtomicU64::new(0);

/// `alloc_low_page()`: a zeroed page-table page from memblock.
///
/// Page-table pages must be reachable by physical address while we build
/// the tables; memblock hands out identity-mapped low memory, so the
/// physical address is also the virtual one.
fn alloc_pgtable_page() -> u64 {
    let phys = memblock_alloc(PAGE_SIZE, PAGE_SIZE, PAGE_SIZE);
    assert!(phys != 0, "out of memory building page tables");
    // SAFETY: phys is a freshly reserved, identity-mapped page.
    unsafe { core::ptr::write_bytes(phys as *mut u8, 0, PAGE_SIZE as usize) };
    phys
}

/// A page table: 512 entries, read and written by physical address.
#[inline]
fn table(phys: u64) -> *mut u64 {
    phys as *mut u64
}

/// Returns the child table a present entry points at, allocating and
/// linking one if the entry is empty. `flags` go on a newly linked entry.
fn next_level(entry: *mut u64) -> u64 {
    // SAFETY: entry points into an identity-mapped page table.
    unsafe {
        if *entry & _PAGE_PRESENT != 0 {
            return *entry & PTE_PFN_MASK;
        }
        let child = alloc_pgtable_page();
        *entry = child | _PAGE_PRESENT | _PAGE_RW | _PAGE_ACCESSED | _PAGE_DIRTY;
        child
    }
}

/// Maps `[start, end)` (both 2 MiB aligned) with 2 MiB pages and `flags`.
fn map_range_2m(pgd: u64, start: u64, end: u64, flags: u64) {
    let mut addr = start;
    while addr < end {
        // SAFETY: each table is an identity-mapped page we own.
        unsafe {
            let pgd_e = table(pgd).add(pgd_index(addr));
            let pud = next_level(pgd_e);
            let pud_e = table(pud).add(pud_index(addr));
            let pmd = next_level(pud_e);
            let pmd_e = table(pmd).add(pmd_index(addr));
            *pmd_e = addr | flags | _PAGE_PSE;
        }
        addr += PMD_SIZE;
    }
}

/// Maps `[start, end)` (4 KiB aligned) with 4 KiB pages and `flags`.
fn map_range_4k(pgd: u64, start: u64, end: u64, flags: u64) {
    let mut addr = start;
    while addr < end {
        // SAFETY: each table is an identity-mapped page we own.
        unsafe {
            let pgd_e = table(pgd).add(pgd_index(addr));
            let pud = next_level(pgd_e);
            let pud_e = table(pud).add(pud_index(addr));
            let pmd = next_level(pud_e);
            let pmd_e = table(pmd).add(pmd_index(addr));
            let pte = next_level(pmd_e);
            let pte_e = table(pte).add(pte_index(addr));
            *pte_e = addr | flags;
        }
        addr += PAGE_SIZE;
    }
}

/// `init_mem_mapping()`: build the kernel direct map and switch to it.
pub fn init_mem_mapping() {
    let end = page_align(max_pfn() * PAGE_SIZE);
    let pgd = alloc_pgtable_page();

    // The first 2 MiB with 4 KiB pages, so page 0 can be left unmapped;
    // the kernel image and boot data live here.
    map_range_4k(pgd, PAGE_SIZE, PMD_SIZE, PAGE_KERNEL_EXEC);
    // Everything else as a 2 MiB-page direct map.
    map_range_2m(
        pgd,
        PMD_SIZE,
        (end + PMD_SIZE - 1) & !(PMD_SIZE - 1),
        PAGE_KERNEL_EXEC,
    );

    KERNEL_PGD.store(pgd, Ordering::Release);
    memblock_reserve(pgd, PAGE_SIZE);

    // Switch to the new tables and flush the TLB. Keep global pages if the
    // boot CPU enabled them (CR4.PGE, set by startup_32's large pages).
    let pge = read_cr4() & (1 << 7);
    // SAFETY: pgd is a complete 4-level mapping of all RAM plus the kernel,
    // so execution and the stack stay mapped across the CR3 load.
    unsafe {
        core::arch::asm!(
            "mov {cr4_off}, %cr4",
            "mov {pgd}, %cr3",
            "mov {cr4}, %cr4",
            cr4_off = in(reg) read_cr4() & !(1u64 << 7),
            pgd = in(reg) pgd,
            cr4 = in(reg) read_cr4() | pge,
            options(att_syntax, nostack, preserves_flags),
        );
    }
}

/// The physical address of the kernel PGD (`init_top_pgt`).
pub fn kernel_pgd() -> u64 {
    KERNEL_PGD.load(Ordering::Acquire)
}

/// Like [`next_level`], but takes intermediate tables from the buddy
/// allocator, for mapping after boot (e.g. vmalloc).
fn next_level_buddy(entry: *mut u64) -> u64 {
    use crate::mm::__GFP_ZERO;
    use crate::mm::page_alloc::alloc_page;
    // SAFETY: entry points into an identity-mapped kernel page table.
    unsafe {
        if *entry & _PAGE_PRESENT != 0 {
            return *entry & PTE_PFN_MASK;
        }
        let child = alloc_page(__GFP_ZERO);
        assert!(child != 0, "out of memory extending kernel page tables");
        *entry = child | _PAGE_PRESENT | _PAGE_RW | _PAGE_ACCESSED | _PAGE_DIRTY;
        child
    }
}

/// `set_pte_at()` for one 4 KiB kernel page: map `va` to `phys` with
/// `flags`, allocating intermediate tables from the buddy allocator.
///
/// # Safety
///
/// `va` must be in a kernel-owned region (e.g. vmalloc space), not aliasing
/// the direct map.
pub unsafe fn map_kernel_page(va: u64, phys: u64, flags: u64) {
    let pgd = kernel_pgd();
    // SAFETY: the kernel PGD and the tables below it are identity-mapped.
    unsafe {
        let pgd_e = table(pgd).add(pgd_index(va));
        let pud = next_level_buddy(pgd_e);
        let pud_e = table(pud).add(pud_index(va));
        let pmd = next_level_buddy(pud_e);
        let pmd_e = table(pmd).add(pmd_index(va));
        let pte = next_level_buddy(pmd_e);
        let pte_e = table(pte).add(pte_index(va));
        *pte_e = (phys & PTE_PFN_MASK) | flags;
        flush_tlb_one(va);
    }
}

/// `pte_clear()` for one kernel page: unmap `va`.
///
/// # Safety
///
/// `va` must have been mapped by [`map_kernel_page`].
pub unsafe fn unmap_kernel_page(va: u64) {
    let pgd = kernel_pgd();
    // SAFETY: walking the identity-mapped kernel tables; the leaf exists.
    unsafe {
        let pgd_e = table(pgd).add(pgd_index(va));
        if *pgd_e & _PAGE_PRESENT == 0 {
            return;
        }
        let pud_e = table(*pgd_e & PTE_PFN_MASK).add(pud_index(va));
        if *pud_e & _PAGE_PRESENT == 0 {
            return;
        }
        let pmd_e = table(*pud_e & PTE_PFN_MASK).add(pmd_index(va));
        if *pmd_e & _PAGE_PRESENT == 0 {
            return;
        }
        let pte_e = table(*pmd_e & PTE_PFN_MASK).add(pte_index(va));
        *pte_e = 0;
        flush_tlb_one(va);
    }
}

/// `__flush_tlb_one_kernel()`: invalidate one page's TLB entry.
fn flush_tlb_one(va: u64) {
    // SAFETY: invlpg only affects the TLB.
    unsafe {
        core::arch::asm!("invlpg ({})", in(reg) va, options(att_syntax, nostack, preserves_flags))
    }
}

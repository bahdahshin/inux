// SPDX-License-Identifier: GPL-2.0
//! Per-task user address spaces.
//!
//! A simplified port of the user-mapping side of `arch/x86/mm/` and
//! `mm/memory.c`. Each user task gets its own PGD: a copy of the kernel PGD
//! (so the kernel stays mapped while the task runs and across the
//! syscall/interrupt entry) plus user pages mapped in a high PGD slot that
//! the kernel's own identity map never uses.
//!
//! rinux runs the kernel identity-mapped in low memory, so user programs
//! are linked at [`USER_LOAD_BASE`], well above RAM, where their page-table
//! slot cannot collide with the kernel's. A real high-half kernel (and thus
//! unmodified user binaries at 0x400000) is future work.
//!
//! All page-table pages come from the buddy allocator and are identity
//! mapped, so a physical address doubles as the pointer to the table.

use crate::arch::x86::include::page::PAGE_SIZE;
use crate::arch::x86::include::pgtable::*;
use crate::arch::x86::mm::init::kernel_pgd;
use crate::mm::__GFP_ZERO;
use crate::mm::page_alloc::alloc_page;

/// Base virtual address user programs are linked at (16 TiB: PGD slot 32,
/// which the kernel never maps).
pub const USER_LOAD_BASE: u64 = 0x0000_1000_0000_0000;
/// Top of the user stack (grows down). Same PGD slot as the code.
pub const USER_STACK_TOP: u64 = 0x0000_1000_4000_0000;
/// Default user stack size.
pub const USER_STACK_SIZE: u64 = 64 * 1024;

#[inline]
fn table(phys: u64) -> *mut u64 {
    phys as *mut u64
}

/// Walks to the next level, allocating a user-accessible table if absent.
fn next_level(entry: *mut u64) -> u64 {
    // SAFETY: entry points into an identity-mapped page table.
    unsafe {
        if *entry & _PAGE_PRESENT != 0 {
            return *entry & PTE_PFN_MASK;
        }
        let child = alloc_page(__GFP_ZERO);
        assert!(child != 0, "out of memory building a user page table");
        *entry = child | _PAGE_PRESENT | _PAGE_RW | _PAGE_USER | _PAGE_ACCESSED | _PAGE_DIRTY;
        child
    }
}

/// `pgd_alloc()` for a user process: a fresh PGD sharing the kernel's
/// entries. Returns its physical address.
pub fn create_user_pgd() -> u64 {
    let pgd = alloc_page(__GFP_ZERO);
    assert!(pgd != 0, "out of memory allocating a user PGD");
    let kpgd = kernel_pgd();
    // SAFETY: both are identity-mapped PGD pages of 512 u64 entries.
    unsafe {
        core::ptr::copy_nonoverlapping(table(kpgd), table(pgd), PTRS_PER_PGD);
    }
    pgd
}

/// Maps one user page `va -> phys` in `pgd` with `flags` (which should
/// include `_PAGE_USER`), creating intermediate tables as needed.
pub fn map_user_page(pgd: u64, va: u64, phys: u64, flags: u64) {
    // SAFETY: pgd and every table below it are identity-mapped pages we own.
    unsafe {
        let pgd_e = table(pgd).add(pgd_index(va));
        let pud = next_level(pgd_e);
        let pud_e = table(pud).add(pud_index(va));
        let pmd = next_level(pud_e);
        let pmd_e = table(pmd).add(pmd_index(va));
        let pte = next_level(pmd_e);
        let pte_e = table(pte).add(pte_index(va));
        *pte_e = (phys & PTE_PFN_MASK) | flags;
    }
}

/// Allocates `len` bytes of zeroed user memory at `va` (page-aligned down)
/// in `pgd`, returning the first physical page. Used by the ELF loader and
/// for the user stack.
pub fn alloc_user_range(pgd: u64, va: u64, len: u64, flags: u64) {
    let start = va & !(PAGE_SIZE - 1);
    let end = (va + len + PAGE_SIZE - 1) & !(PAGE_SIZE - 1);
    let mut addr = start;
    while addr < end {
        let page = alloc_page(__GFP_ZERO);
        assert!(page != 0, "out of memory for a user mapping");
        map_user_page(pgd, addr, page, flags);
        addr += PAGE_SIZE;
    }
}

/// Walks to the PTE for `va` without allocating, or `None` if an intermediate
/// table is absent (the page was never mapped).
fn user_pte(pgd: u64, va: u64) -> Option<*mut u64> {
    // SAFETY: pgd and every table below it are identity-mapped pages we own.
    unsafe {
        let mut e = table(pgd).add(pgd_index(va));
        for idx in [pud_index(va), pmd_index(va), pte_index(va)] {
            if *e & _PAGE_PRESENT == 0 {
                return None;
            }
            e = table(*e & PTE_PFN_MASK).add(idx);
        }
        Some(e)
    }
}

/// `invlpg`: drop the TLB entry for `va`; the user PGD is the live CR3 while
/// its task is in a syscall, so a changed PTE must be flushed.
fn invlpg(va: u64) {
    // SAFETY: invalidating a TLB entry has no effect beyond a later refill.
    unsafe {
        core::arch::asm!("invlpg [{}]", in(reg) va, options(nostack, preserves_flags));
    }
}

/// Unmaps the user page at `va` in `pgd`, returning its physical frame for
/// the caller to free, or `None` if nothing was mapped there. A `PROT_NONE`
/// page (not present but holding a frame) counts as mapped.
pub fn unmap_user_page(pgd: u64, va: u64) -> Option<u64> {
    let e = user_pte(pgd, va)?;
    // SAFETY: e points at a PTE in an identity-mapped table we own.
    unsafe {
        let v = *e;
        if v & PTE_PFN_MASK == 0 {
            return None;
        }
        *e = 0;
        invlpg(va);
        Some(v & PTE_PFN_MASK)
    }
}

/// Rewrites the flags of the mapped user page at `va`, keeping its frame.
/// Returns false if no page is mapped there.
pub fn set_user_page_flags(pgd: u64, va: u64, flags: u64) -> bool {
    let Some(e) = user_pte(pgd, va) else {
        return false;
    };
    // SAFETY: as unmap_user_page.
    unsafe {
        let v = *e;
        if v & PTE_PFN_MASK == 0 {
            return false;
        }
        *e = (v & PTE_PFN_MASK) | flags;
        invlpg(va);
    }
    true
}

/// Copies `data` into already-mapped user memory at `va` in `pgd`.
///
/// Walks the page tables to reach each physical page (identity mapped) and
/// copies, so it works regardless of the current CR3.
///
/// # Safety
///
/// `[va, va+data.len())` must be mapped writable in `pgd`.
pub unsafe fn copy_to_user_pgd(pgd: u64, va: u64, data: &[u8]) {
    let mut off = 0usize;
    while off < data.len() {
        let cur = va + off as u64;
        let page_off = (cur & (PAGE_SIZE - 1)) as usize;
        let n = (PAGE_SIZE as usize - page_off).min(data.len() - off);
        // SAFETY: the caller guarantees cur is mapped; walk to its phys page.
        let phys = unsafe { walk(pgd, cur) };
        // SAFETY: phys is identity-mapped RAM backing this user page.
        unsafe { core::ptr::copy_nonoverlapping(data.as_ptr().add(off), phys as *mut u8, n) };
        off += n;
    }
}

/// Returns the physical address `va` maps to in `pgd`.
///
/// # Safety
///
/// `va` must be mapped in `pgd`.
unsafe fn walk(pgd: u64, va: u64) -> u64 {
    // SAFETY: the caller guarantees va is mapped; every table is identity
    // mapped.
    unsafe {
        let pud = *table(pgd).add(pgd_index(va)) & PTE_PFN_MASK;
        let pmd = *table(pud).add(pud_index(va)) & PTE_PFN_MASK;
        let pte = *table(pmd).add(pmd_index(va)) & PTE_PFN_MASK;
        let entry = *table(pte).add(pte_index(va));
        (entry & PTE_PFN_MASK) | (va & (PAGE_SIZE - 1))
    }
}

/// Loads `pgd` into CR3, switching the active address space.
///
/// # Safety
///
/// `pgd` must be a complete page table (kernel entries present) that keeps
/// the current instruction and stack mapped.
pub unsafe fn load_cr3(pgd: u64) {
    // SAFETY: the caller guarantees pgd maps the running kernel context.
    unsafe {
        core::arch::asm!("mov {}, %cr3", in(reg) pgd, options(att_syntax, nostack, preserves_flags))
    }
}

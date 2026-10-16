// SPDX-License-Identifier: GPL-2.0
//! Page constants and physical/virtual address conversion.
//!
//! Ported from `include/vdso/page.h`, `arch/x86/include/asm/page_types.h`
//! and `arch/x86/include/asm/page_64.h`.
//!
//! rinux runs identity-mapped: the kernel is loaded at its physical link
//! address, so for now `__pa()` and `__va()` are the identity and
//! `PAGE_OFFSET` is 0. When the kernel moves to the top of the address
//! space this becomes the Linux direct map at `0xffff888000000000`.

/// `PAGE_SHIFT`.
pub const PAGE_SHIFT: u32 = 12;
/// `PAGE_SIZE`.
pub const PAGE_SIZE: u64 = 1 << PAGE_SHIFT;
/// `PAGE_MASK`: clears the offset within a page.
pub const PAGE_MASK: u64 = !(PAGE_SIZE - 1);

/// `PAGE_OFFSET`: base of the direct map (identity for now).
pub const PAGE_OFFSET: u64 = 0;

/// `PAGE_ALIGN()`: round up to a page boundary.
#[inline]
pub const fn page_align(x: u64) -> u64 {
    (x + PAGE_SIZE - 1) & PAGE_MASK
}

/// `PAGE_ALIGN_DOWN()`.
#[inline]
pub const fn page_align_down(x: u64) -> u64 {
    x & PAGE_MASK
}

/// `__pa()`: virtual (kernel) address to physical.
#[inline]
pub const fn pa(va: u64) -> u64 {
    va - PAGE_OFFSET
}

/// `__va()`: physical address to virtual (kernel) address.
#[inline]
pub const fn va(pa: u64) -> u64 {
    pa + PAGE_OFFSET
}

/// `PFN_DOWN()`: the page frame number containing `addr`.
#[inline]
pub const fn pfn_down(addr: u64) -> u64 {
    addr >> PAGE_SHIFT
}

/// `PFN_UP()`: the first page frame at or after `addr`.
#[inline]
pub const fn pfn_up(addr: u64) -> u64 {
    (addr + PAGE_SIZE - 1) >> PAGE_SHIFT
}

/// `PFN_PHYS()`: the physical address of a page frame.
#[inline]
pub const fn pfn_phys(pfn: u64) -> u64 {
    pfn << PAGE_SHIFT
}

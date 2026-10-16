// SPDX-License-Identifier: GPL-2.0-only
//! Non-contiguous virtual memory: `vmalloc`/`vfree`.
//!
//! A partial port of `mm/vmalloc.c`. `vmalloc()` allocates physically
//! scattered pages from the buddy allocator and maps them into a
//! contiguous run of a dedicated virtual region. The busy areas are kept in
//! a red-black tree of `vmap_area`s ordered by address, and a new area is
//! placed in the first gap large enough (first fit), as `alloc_vmap_area()`
//! does with the free tree.
//!
//! Simplified from Linux's: one global tree under a lock instead of the
//! separate free/busy trees and per-CPU vmap caches, no guard pages between
//! areas, no huge mappings, and `vmap()`/`ioremap()` are not ported.
//!
//! Copyright (C) 1993  Linus Torvalds

use rinux_lib::rbtree::{RbNode, RbRoot, erase, first, insert_color, link_node, next};

use crate::arch::x86::include::page::{PAGE_SHIFT, PAGE_SIZE, page_align};
use crate::arch::x86::include::pgtable::PAGE_KERNEL;
use crate::arch::x86::mm::init::{map_kernel_page, unmap_kernel_page};
use crate::kernel::locking::SpinLock;
use crate::mm::__GFP_ZERO;
use crate::mm::page_alloc::{alloc_page, free_page};
use crate::mm::slub::{kfree, kmalloc};

/// `VMALLOC_START`: a high virtual region outside the direct map. The
/// kernel PGD covers the whole 48-bit space; these addresses fault until
/// vmalloc maps them.
pub const VMALLOC_START: u64 = 0xffff_c900_0000_0000;
/// `VMALLOC_END`: 32 TiB of vmalloc space, as on x86-64.
pub const VMALLOC_END: u64 = 0xffff_e8ff_ffff_ffff;

/// `struct vmap_area`, with the physical pages backing it.
#[repr(C)]
struct VmapArea {
    rb: RbNode,
    va_start: u64,
    va_end: u64,
    /// Physical addresses of the backing pages (`area->pages`).
    pages: *mut u64,
    nr_pages: usize,
}

const RB_OFFSET: usize = core::mem::offset_of!(VmapArea, rb);

unsafe fn area_of(rb: *mut RbNode) -> *mut VmapArea {
    // SAFETY: rb is the rb field of a live VmapArea.
    unsafe { (rb as *mut u8).sub(RB_OFFSET) as *mut VmapArea }
}

/// The vmap tree. `RbRoot` holds raw pointers; the lock makes access to
/// them exclusive, so wrap it to mark it `Send`.
struct Tree(RbRoot);
// SAFETY: every access is serialized by VMAP_TREE's lock.
unsafe impl Send for Tree {}

static VMAP_TREE: SpinLock<Tree> = SpinLock::new(Tree(RbRoot::new()));

/// Inserts `area` into the address-ordered tree.
unsafe fn insert_area(root: *mut RbRoot, area: *mut VmapArea) {
    // SAFETY: valid tree; area is pinned by kmalloc.
    unsafe {
        let mut link: *mut *mut RbNode = &mut (*root).rb_node;
        let mut parent: *mut RbNode = core::ptr::null_mut();
        while !(*link).is_null() {
            parent = *link;
            let pstart = (*area_of(parent)).va_start;
            link = if (*area).va_start < pstart {
                &mut (*parent).rb_left
            } else {
                &mut (*parent).rb_right
            };
        }
        link_node(&mut (*area).rb, parent, link);
        insert_color(&mut (*area).rb, root);
    }
}

/// `alloc_vmap_area()`: the first gap of at least `size` in the tree.
unsafe fn find_free_range(root: *mut RbRoot, size: u64) -> u64 {
    // SAFETY: valid tree.
    unsafe {
        let mut addr = VMALLOC_START;
        let mut n = first(root);
        while !n.is_null() {
            let area = area_of(n);
            if (*area).va_start >= addr + size {
                break; // gap [addr, va_start) is big enough
            }
            addr = addr.max((*area).va_end);
            n = next(n);
        }
        if addr + size <= VMALLOC_END { addr } else { 0 }
    }
}

/// `vmalloc()`: allocate `size` bytes of virtually contiguous memory.
///
/// Returns the virtual address, or 0. The memory is zeroed.
pub fn vmalloc(size: usize) -> u64 {
    if size == 0 {
        return 0;
    }
    let size = page_align(size as u64);
    let nr_pages = (size >> PAGE_SHIFT) as usize;

    // The area descriptor and its page array.
    let area = kmalloc(core::mem::size_of::<VmapArea>(), __GFP_ZERO) as *mut VmapArea;
    if area.is_null() {
        return 0;
    }
    let pages = kmalloc(nr_pages * 8, __GFP_ZERO) as *mut u64;
    if pages.is_null() {
        kfree(area as u64);
        return 0;
    }

    let va_start = {
        let mut tree = VMAP_TREE.lock_irqsave();
        // SAFETY: valid tree.
        unsafe { find_free_range(&mut tree.0, size) }
    };
    if va_start == 0 {
        kfree(pages as u64);
        kfree(area as u64);
        return 0;
    }

    // Allocate and map the backing pages one at a time.
    for i in 0..nr_pages {
        let phys = alloc_page(__GFP_ZERO);
        if phys == 0 {
            // Roll back.
            // SAFETY: the first i pages were mapped and recorded.
            unsafe { unmap_and_free(va_start, pages, i) };
            kfree(pages as u64);
            kfree(area as u64);
            return 0;
        }
        // SAFETY: pages has nr_pages slots; map the page into vmalloc space.
        unsafe {
            pages.add(i).write(phys);
            map_kernel_page(va_start + (i as u64) * PAGE_SIZE, phys, PAGE_KERNEL);
        }
    }

    // SAFETY: area is pinned; publish it in the tree.
    unsafe {
        (*area).va_start = va_start;
        (*area).va_end = va_start + size;
        (*area).pages = pages;
        (*area).nr_pages = nr_pages;
        let mut tree = VMAP_TREE.lock_irqsave();
        insert_area(&mut tree.0, area);
    }
    va_start
}

/// Unmaps and frees the first `n` backing pages of an area at `va_start`.
unsafe fn unmap_and_free(va_start: u64, pages: *mut u64, n: usize) {
    // SAFETY: the first n pages are mapped and recorded.
    unsafe {
        for i in 0..n {
            let va = va_start + (i as u64) * PAGE_SIZE;
            unmap_kernel_page(va);
            free_page(pages.add(i).read());
        }
    }
}

/// `vfree()`: free memory from [`vmalloc`].
pub fn vfree(addr: u64) {
    if addr == 0 {
        return;
    }
    // Find and remove the area.
    let area = {
        let mut tree = VMAP_TREE.lock_irqsave();
        // SAFETY: valid tree.
        unsafe {
            let root = &mut tree.0 as *mut RbRoot;
            let mut n = first(root);
            let mut found: *mut VmapArea = core::ptr::null_mut();
            while !n.is_null() {
                if (*area_of(n)).va_start == addr {
                    found = area_of(n);
                    break;
                }
                n = next(n);
            }
            if found.is_null() {
                pr_err!("vfree: bad address {:#x}", addr);
                return;
            }
            erase(&mut (*found).rb, root);
            found
        }
    };
    // SAFETY: area was just removed from the tree; free its pages and itself.
    unsafe {
        unmap_and_free((*area).va_start, (*area).pages, (*area).nr_pages);
        kfree((*area).pages as u64);
        kfree(area as u64);
    }
}

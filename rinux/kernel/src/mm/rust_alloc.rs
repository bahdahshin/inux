// SPDX-License-Identifier: GPL-2.0
//! The Rust global allocator, backed by `kmalloc`.
//!
//! This is what lets the kernel use `alloc`: `Box`, `Vec`, `String` and the
//! rest route their allocations through [`kmalloc`]/[`kfree`]. It plays the
//! role of `rust/kernel/alloc/allocator.rs` in Linux's Rust support.
//!
//! `kmalloc` returns memory aligned to its (power-of-two) size class, so an
//! allocation asking for alignment `a` is served from a cache of at least
//! `a` bytes, which is then `a`-aligned. Alignments above a page are not
//! supported.

use core::alloc::{GlobalAlloc, Layout};

use crate::mm::GFP_KERNEL;
use crate::mm::slub::{kfree, kmalloc};

/// The kernel's global allocator.
pub struct KernelAllocator;

unsafe impl GlobalAlloc for KernelAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // Fold the alignment into the size so the chosen cache is aligned.
        let size = layout.size().max(layout.align());
        kmalloc(size, GFP_KERNEL) as *mut u8
    }

    unsafe fn dealloc(&self, ptr: *mut u8, _layout: Layout) {
        // kfree recovers the size from the page-kind table.
        kfree(ptr as u64);
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let size = layout.size().max(layout.align());
        kmalloc(size, GFP_KERNEL | crate::mm::__GFP_ZERO) as *mut u8
    }
}

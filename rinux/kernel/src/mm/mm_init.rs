// SPDX-License-Identifier: GPL-2.0-only
//! Bringing up the memory allocators.
//!
//! A partial port of `mm/mm_init.c` and the hand-off in `mm/memblock.c`
//! (`memblock_free_all()`): allocate the `struct page` array, then release
//! every free memblock range into the buddy allocator. Linux's sparsemem,
//! zone initialization, per-CPU setup and deferred init are not ported.

use core::mem::size_of;

use crate::arch::x86::include::page::{PAGE_SIZE, pfn_phys};
use crate::mm::memblock::{
    alloc as memblock_alloc, for_each_free_region, max_pfn, memory_size, reserved_size,
};
use crate::mm::page_alloc::{Page, free_pages_range, init_mem_map, nr_free_pages};
use crate::mm::slub::kmem_cache_init;

/// `mm_core_init()`: set up the page allocator and the slab allocator.
pub fn mm_core_init() {
    let max_pfn = max_pfn();

    // Allocate the struct page array (mem_map) from memblock.
    let bytes = max_pfn * size_of::<Page>() as u64;
    let mem_map_phys = memblock_alloc(bytes, PAGE_SIZE, PAGE_SIZE);
    assert!(mem_map_phys != 0, "cannot allocate mem_map");
    // SAFETY: the region is reserved and identity-mapped; zero it and hand
    // it to the page allocator.
    unsafe {
        core::ptr::write_bytes(mem_map_phys as *mut u8, 0, bytes as usize);
        init_mem_map(mem_map_phys as *mut Page, max_pfn);
    }

    // memblock_free_all(): release free RAM into the buddy allocator.
    for_each_free_region(|start, end| {
        free_pages_range(start >> 12, end >> 12);
    });

    let total_kb = memory_size() / 1024;
    let avail_kb = nr_free_pages() * PAGE_SIZE / 1024;
    let reserved_kb = reserved_size() / 1024;
    pr_info!(
        "Memory: {}K/{}K available ({}K reserved)",
        avail_kb,
        total_kb,
        reserved_kb
    );
    let _ = pfn_phys(0);

    // kmem_cache_init(): the kmalloc slab caches.
    kmem_cache_init();
    pr_info!("mm: page allocator and kmalloc up");
}

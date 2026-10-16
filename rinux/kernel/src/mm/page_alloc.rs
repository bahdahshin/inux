// SPDX-License-Identifier: GPL-2.0-only
//! The buddy page allocator.
//!
//! A partial port of `mm/page_alloc.c` with the `struct page` array set up
//! by `mm/mm_init.c`. Free physical pages are kept in per-order free lists
//! (`free_area[order]`); an allocation splits the smallest big-enough block
//! and a free coalesces a block with its buddy, which is the classic buddy
//! algorithm of `__rmqueue_smallest()`/`expand()` and `__free_one_page()`.
//!
//! Simplified from Linux's: one node, one zone and one migratetype, no
//! per-CPU page lists, watermarks, reclaim, compaction, CMA, guard pages or
//! page flags beyond "on the buddy allocator". `GFP` flags other than
//! `__GFP_ZERO` are accepted and ignored.
//!
//! Copyright (C) 1991, 1992, 1993, 1994  Linus Torvalds

use core::sync::atomic::{AtomicU64, Ordering};

use rinux_lib::list::{ListHead, list_add, list_del, list_entry};

use crate::arch::x86::include::page::{PAGE_SHIFT, PAGE_SIZE, pfn_phys};
use crate::kernel::locking::SpinLock;
use crate::mm::{__GFP_ZERO, Gfp};

/// `MAX_PAGE_ORDER`: the largest block is 2^10 pages = 4 MiB.
pub const MAX_PAGE_ORDER: usize = 10;
/// `NR_PAGE_ORDERS`.
pub const NR_PAGE_ORDERS: usize = MAX_PAGE_ORDER + 1;

/// `PG_buddy`: the page is free and managed by the buddy allocator. Stored
/// in [`Page::flags`]; the page's order lives in [`Page::order`].
const PG_BUDDY: u64 = 1 << 0;

/// `struct page`.
#[repr(C)]
pub struct Page {
    /// Free-list linkage (`page->lru`) while on a free list.
    lru: ListHead,
    flags: u64,
    /// Buddy order while free.
    order: u32,
    _pad: u32,
}

const LRU_OFFSET: usize = core::mem::offset_of!(Page, lru);

/// `struct free_area`.
struct FreeArea {
    free_list: ListHead,
    nr_free: u64,
}

struct Zone {
    free_area: [FreeArea; NR_PAGE_ORDERS],
    /// Base of the `struct page` array and the pfn it starts at.
    mem_map: *mut Page,
    max_pfn: u64,
}

// SAFETY: every access goes through the ZONE spinlock.
unsafe impl Send for Zone {}

const fn empty_free_area() -> FreeArea {
    FreeArea {
        free_list: ListHead::new(),
        nr_free: 0,
    }
}

static ZONE: SpinLock<Zone> = SpinLock::new(Zone {
    free_area: [const { empty_free_area() }; NR_PAGE_ORDERS],
    mem_map: core::ptr::null_mut(),
    max_pfn: 0,
});

/// Total free pages, for reporting (`vm_stat` free count).
static NR_FREE_PAGES: AtomicU64 = AtomicU64::new(0);
/// Pages the allocator manages (`totalram_pages`).
static TOTALRAM_PAGES: AtomicU64 = AtomicU64::new(0);

impl Zone {
    /// `pfn_to_page()`.
    #[inline]
    fn page(&self, pfn: u64) -> *mut Page {
        // SAFETY: callers only pass pfns below max_pfn, which mem_map covers.
        unsafe { self.mem_map.add(pfn as usize) }
    }

    /// `page_to_pfn()`.
    #[inline]
    fn pfn(&self, page: *mut Page) -> u64 {
        // SAFETY: page is within the mem_map array.
        (unsafe { page.offset_from(self.mem_map) }) as u64
    }

    #[inline]
    fn is_buddy(&self, pfn: u64, order: u32) -> bool {
        if pfn >= self.max_pfn {
            return false;
        }
        let p = self.page(pfn);
        // SAFETY: pfn < max_pfn, so p is a valid Page.
        unsafe { (*p).flags & PG_BUDDY != 0 && (*p).order == order }
    }

    fn add_to_free_list(&mut self, page: *mut Page, order: u32) {
        // SAFETY: page is a valid Page we own; set it up as a free block.
        unsafe {
            (*page).flags |= PG_BUDDY;
            (*page).order = order;
            list_add(
                &mut (*page).lru,
                &mut self.free_area[order as usize].free_list,
            );
        }
        self.free_area[order as usize].nr_free += 1;
    }

    fn del_from_free_list(&mut self, page: *mut Page, order: u32) {
        // SAFETY: page is currently on free_area[order].
        unsafe {
            list_del(&mut (*page).lru);
            (*page).flags &= !PG_BUDDY;
        }
        self.free_area[order as usize].nr_free -= 1;
    }

    /// `expand()`: after taking a block of `high` for a request of `low`,
    /// return the unused upper halves to the smaller free lists.
    fn expand(&mut self, page: *mut Page, low: u32, mut high: u32) {
        let base_pfn = self.pfn(page);
        let mut size = 1u64 << high;
        while high > low {
            high -= 1;
            size >>= 1;
            let buddy = self.page(base_pfn + size);
            self.add_to_free_list(buddy, high);
        }
    }

    /// `__rmqueue_smallest()`: the smallest free block of at least `order`.
    fn rmqueue_smallest(&mut self, order: u32) -> *mut Page {
        for current in order as usize..NR_PAGE_ORDERS {
            if self.free_area[current].nr_free == 0 {
                continue;
            }
            // SAFETY: the list is non-empty, so next is a Page's lru link.
            let page = unsafe {
                let head = &mut self.free_area[current].free_list as *mut ListHead;
                list_entry::<Page>((*head).next, LRU_OFFSET)
            };
            self.del_from_free_list(page, current as u32);
            self.expand(page, order, current as u32);
            return page;
        }
        core::ptr::null_mut()
    }

    /// `__free_one_page()`: free a block, merging with its buddy upward.
    fn free_one_page(&mut self, mut page: *mut Page, mut order: u32) {
        let mut pfn = self.pfn(page);
        while (order as usize) < MAX_PAGE_ORDER {
            let buddy_pfn = pfn ^ (1 << order);
            if !self.is_buddy(buddy_pfn, order) {
                break;
            }
            let buddy = self.page(buddy_pfn);
            self.del_from_free_list(buddy, order);
            let combined = buddy_pfn & pfn;
            page = self.page(combined);
            pfn = combined;
            order += 1;
        }
        self.add_to_free_list(page, order);
    }
}

/// `__alloc_pages()`: allocate a block of `2^order` physically contiguous
/// pages. Returns the physical address of the first page, or 0.
pub fn alloc_pages(gfp: Gfp, order: u32) -> u64 {
    let page = {
        let mut zone = ZONE.lock_irqsave();
        let page = zone.rmqueue_smallest(order);
        if page.is_null() {
            return 0;
        }
        NR_FREE_PAGES.fetch_sub(1 << order, Ordering::Relaxed);
        zone.pfn(page)
    };
    let phys = pfn_phys(page);
    if gfp & __GFP_ZERO != 0 {
        // SAFETY: the pages are now owned by this allocation and mapped.
        unsafe { core::ptr::write_bytes(phys as *mut u8, 0, (PAGE_SIZE as usize) << order) };
    }
    phys
}

/// `alloc_page()`: one page.
pub fn alloc_page(gfp: Gfp) -> u64 {
    alloc_pages(gfp, 0)
}

/// `__free_pages()`: free a block allocated with [`alloc_pages`].
pub fn free_pages(phys: u64, order: u32) {
    let pfn = phys >> PAGE_SHIFT;
    let mut zone = ZONE.lock_irqsave();
    let page = zone.page(pfn);
    zone.free_one_page(page, order);
    NR_FREE_PAGES.fetch_add(1 << order, Ordering::Relaxed);
}

/// `free_page()`: one page.
pub fn free_page(phys: u64) {
    free_pages(phys, 0);
}

/// Sets the `struct page` array; called by `mm_init` once it is allocated.
///
/// # Safety
///
/// `mem_map` must point at `max_pfn` zeroed, pinned `Page`s.
pub unsafe fn init_mem_map(mem_map: *mut Page, max_pfn: u64) {
    let mut zone = ZONE.lock_irqsave();
    // SAFETY: the caller guarantees the array; initialize the free lists.
    unsafe {
        for fa in zone.free_area.iter_mut() {
            fa.free_list.init();
        }
    }
    zone.mem_map = mem_map;
    zone.max_pfn = max_pfn;
}

/// Frees a run of pages `[start_pfn, end_pfn)` into the buddy allocator at
/// boot, the largest aligned blocks first (`__free_pages_memory()`).
pub fn free_pages_range(start_pfn: u64, end_pfn: u64) {
    let mut pfn = start_pfn;
    while pfn < end_pfn {
        // Largest order that stays aligned and in range.
        let mut order = MAX_PAGE_ORDER as u32;
        while order > 0 && (pfn & ((1 << order) - 1) != 0 || pfn + (1 << order) > end_pfn) {
            order -= 1;
        }
        {
            let mut zone = ZONE.lock_irqsave();
            let page = zone.page(pfn);
            zone.free_one_page(page, order);
        }
        NR_FREE_PAGES.fetch_add(1 << order, Ordering::Relaxed);
        TOTALRAM_PAGES.fetch_add(1 << order, Ordering::Relaxed);
        pfn += 1 << order;
    }
}

/// `nr_free_pages()`.
pub fn nr_free_pages() -> u64 {
    NR_FREE_PAGES.load(Ordering::Relaxed)
}

/// `totalram_pages()`.
pub fn totalram_pages() -> u64 {
    TOTALRAM_PAGES.load(Ordering::Relaxed)
}

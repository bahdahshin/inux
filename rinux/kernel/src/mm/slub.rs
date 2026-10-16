// SPDX-License-Identifier: GPL-2.0
//! The slab allocator: `kmalloc`/`kfree`.
//!
//! A partial port of `mm/slub.c` and `mm/slab_common.c`. kmalloc has an
//! array of caches for power-of-two sizes; each cache carves pages from the
//! buddy allocator into equal objects threaded on a free list, as SLUB
//! does. Allocations larger than a page go straight to the page allocator
//! (`kmalloc_large()`).
//!
//! The size of a freed object is recovered from a side table indexed by
//! page frame (`PAGE_KIND`), which records the owning cache or, for large
//! allocations, the order. Linux keeps this in `struct slab`; the side
//! table avoids enlarging `struct page`.
//!
//! Simplified from Linux's: no per-CPU slabs, no NUMA, no red-zoning or
//! poisoning, no cache merging, and freed slab pages are not returned to
//! the buddy allocator (documented debt).

use core::sync::atomic::{AtomicU64, Ordering};

use crate::arch::x86::include::page::{PAGE_SHIFT, PAGE_SIZE};
use crate::kernel::locking::{RacyCell, SpinLock};
use crate::mm::page_alloc::{alloc_pages, free_pages};
use crate::mm::{__GFP_ZERO, Gfp};

/// `kmalloc` size classes: 8 bytes up to one page.
const KMALLOC_SIZES: [usize; 10] = [8, 16, 32, 64, 128, 256, 512, 1024, 2048, 4096];
const NR_CACHES: usize = KMALLOC_SIZES.len();

/// `PAGE_KIND` marker: the page belongs to no slab.
const KIND_NONE: u8 = 0;
/// A large kmalloc of `2^order` pages is marked `KIND_LARGE | order`.
const KIND_LARGE: u8 = 0x80;

/// Per-page-frame ownership, set up by [`kmem_cache_init`].
static PAGE_KIND: RacyCell<PageKind> = RacyCell::new(PageKind {
    table: core::ptr::null_mut(),
    max_pfn: 0,
});

struct PageKind {
    table: *mut u8,
    max_pfn: u64,
}

// SAFETY: the table is written only under the matching cache lock or during
// single-threaded init.
unsafe impl Sync for PageKind {}

fn set_kind(pfn: u64, kind: u8) {
    // SAFETY: pfn < max_pfn, table is a max_pfn-byte array.
    unsafe {
        let pk = &*PAGE_KIND.get();
        if pfn < pk.max_pfn {
            pk.table.add(pfn as usize).write(kind);
        }
    }
}

fn get_kind(pfn: u64) -> u8 {
    // SAFETY: as for set_kind.
    unsafe {
        let pk = &*PAGE_KIND.get();
        if pfn < pk.max_pfn {
            pk.table.add(pfn as usize).read()
        } else {
            KIND_NONE
        }
    }
}

/// One free object: its first word links to the next free object.
#[repr(C)]
struct FreeObj {
    next: *mut FreeObj,
}

/// `struct kmem_cache`.
struct Cache {
    size: usize,
    free: *mut FreeObj,
}

// SAFETY: accessed only under the cache's lock.
unsafe impl Send for Cache {}

static CACHES: [SpinLock<Cache>; NR_CACHES] = {
    // One SpinLock<Cache> per size class, size filled in at init.
    [const {
        SpinLock::new(Cache {
            size: 0,
            free: core::ptr::null_mut(),
        })
    }; NR_CACHES]
};

/// `kmalloc` statistics.
static SLAB_PAGES: AtomicU64 = AtomicU64::new(0);

/// The cache index for an allocation of `size`/`align`, or `None` if it is
/// too big and must go to the page allocator.
fn cache_index(size: usize, align: usize) -> Option<usize> {
    let need = size.max(align).max(1);
    KMALLOC_SIZES.iter().position(|&s| s >= need)
}

/// `kmem_cache_init()`: allocate the page-kind table and size the caches.
///
/// The table comes from the buddy allocator, not memblock: by the time this
/// runs, `memblock_free_all()` has already handed memblock's free memory to
/// the buddy allocator, so a late memblock allocation would alias it.
pub fn kmem_cache_init() {
    let max_pfn = crate::mm::memblock::max_pfn();
    let pages = (max_pfn as usize).div_ceil(PAGE_SIZE as usize);
    let order = pages.next_power_of_two().trailing_zeros();
    let table = alloc_pages(__GFP_ZERO, order);
    assert!(table != 0, "cannot allocate slab page-kind table");
    // SAFETY: a freshly allocated, zeroed, identity-mapped block that covers
    // max_pfn bytes (KIND_NONE is 0, so the zeroing is the initial state).
    unsafe {
        *PAGE_KIND.get() = PageKind {
            table: table as *mut u8,
            max_pfn,
        };
    }
    for (i, &size) in KMALLOC_SIZES.iter().enumerate() {
        CACHES[i].lock_irqsave().size = size;
    }
}

/// Carves a fresh page into `size`-byte objects onto the cache free list.
/// Returns false if the page allocator is empty.
fn refill(cache: &mut Cache, idx: usize) -> bool {
    let phys = alloc_pages(0, 0);
    if phys == 0 {
        return false;
    }
    SLAB_PAGES.fetch_add(1, Ordering::Relaxed);
    set_kind(phys >> PAGE_SHIFT, idx as u8 + 1);
    let n = PAGE_SIZE as usize / cache.size;
    // Thread the objects: each points at the next, the last at the old head.
    for k in 0..n {
        let obj = (phys as usize + k * cache.size) as *mut FreeObj;
        // SAFETY: obj is a distinct, aligned slot inside the page.
        unsafe { (*obj).next = cache.free };
        cache.free = obj;
    }
    true
}

/// `kmalloc()`: allocate `size` bytes.
///
/// Returns the virtual address, or 0 on failure. Honours `__GFP_ZERO`.
pub fn kmalloc(size: usize, gfp: Gfp) -> u64 {
    if size == 0 {
        // ZERO_SIZE_PTR: a non-null pointer that must never be dereferenced.
        return 16;
    }
    let addr = match cache_index(size, 1) {
        Some(idx) => {
            let mut cache = CACHES[idx].lock_irqsave();
            if cache.free.is_null() && !refill(&mut cache, idx) {
                return 0;
            }
            let obj = cache.free;
            // SAFETY: obj came off this cache's free list.
            cache.free = unsafe { (*obj).next };
            obj as u64
        }
        None => kmalloc_large(size),
    };
    if addr != 0 && gfp & __GFP_ZERO != 0 {
        // SAFETY: addr owns at least `size` bytes.
        unsafe { core::ptr::write_bytes(addr as *mut u8, 0, size) };
    }
    addr
}

/// `kmalloc_large()`: allocations bigger than a cache go to the buddy
/// allocator.
fn kmalloc_large(size: usize) -> u64 {
    let pages = size.div_ceil(PAGE_SIZE as usize);
    let order = (usize::BITS - (pages - 1).leading_zeros()).min(31);
    let order = if pages == 1 { 0 } else { order };
    let phys = alloc_pages(0, order);
    if phys == 0 {
        return 0;
    }
    set_kind(phys >> PAGE_SHIFT, KIND_LARGE | order as u8);
    phys
}

/// `kfree()`: free memory from [`kmalloc`].
pub fn kfree(ptr: u64) {
    if ptr == 0 || ptr == 16 {
        return;
    }
    let pfn = ptr >> PAGE_SHIFT;
    let kind = get_kind(pfn);
    if kind & KIND_LARGE != 0 {
        let order = (kind & !KIND_LARGE) as u32;
        set_kind(pfn, KIND_NONE);
        free_pages(ptr, order);
        return;
    }
    if kind == KIND_NONE {
        pr_err!("kfree: bad pointer {:#x}", ptr);
        return;
    }
    let idx = (kind - 1) as usize;
    let mut cache = CACHES[idx].lock_irqsave();
    let obj = ptr as *mut FreeObj;
    // SAFETY: ptr came from this cache; push it back on the free list.
    unsafe { (*obj).next = cache.free };
    cache.free = obj;
}

/// `ksize()`: the usable size of a [`kmalloc`] allocation.
pub fn ksize(ptr: u64) -> usize {
    if ptr == 0 || ptr == 16 {
        return 0;
    }
    let kind = get_kind(ptr >> PAGE_SHIFT);
    if kind & KIND_LARGE != 0 {
        return (PAGE_SIZE as usize) << (kind & !KIND_LARGE) as u32;
    }
    if kind == KIND_NONE {
        return 0;
    }
    KMALLOC_SIZES[(kind - 1) as usize]
}

/// Pages held by the slab allocator, for reporting.
pub fn slab_pages() -> u64 {
    SLAB_PAGES.load(Ordering::Relaxed)
}

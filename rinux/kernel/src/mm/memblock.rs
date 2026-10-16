// SPDX-License-Identifier: GPL-2.0
//! The boot-time physical memory allocator.
//!
//! A partial port of `mm/memblock.c`. memblock tracks physical memory as
//! two arrays of regions, `memory` (all RAM) and `reserved` (in use), and
//! hands out memory before the page allocator exists. The region arrays
//! have a fixed size here (`INIT_MEMBLOCK_REGIONS`); Linux grows them with
//! `memblock_double_array()`, which is not ported because the e820 map is
//! far smaller than the limit.
//!
//! Not ported: NUMA nodes, `MEMBLOCK_*` flags beyond reserve, bottom-up
//! allocation, and the hand-off of free memory to the page allocator
//! (`memblock_free_all()`), which `mm_init` does directly.

use crate::arch::x86::include::page::{PAGE_SIZE, page_align, page_align_down};
use crate::kernel::locking::SpinLock;

/// `INIT_MEMBLOCK_REGIONS`.
pub const INIT_MEMBLOCK_REGIONS: usize = 128;

/// `struct memblock_region`.
#[derive(Clone, Copy)]
pub struct Region {
    pub base: u64,
    pub size: u64,
}

const EMPTY: Region = Region { base: 0, size: 0 };

/// `struct memblock_type`.
pub struct MemblockType {
    pub regions: [Region; INIT_MEMBLOCK_REGIONS],
    pub cnt: usize,
    pub total_size: u64,
}

impl MemblockType {
    const fn new() -> MemblockType {
        MemblockType {
            regions: [EMPTY; INIT_MEMBLOCK_REGIONS],
            cnt: 0,
            total_size: 0,
        }
    }

    /// `memblock_insert_region()`: opens a gap at `idx` and fills it.
    fn insert(&mut self, idx: usize, base: u64, size: u64) {
        assert!(
            self.cnt < INIT_MEMBLOCK_REGIONS,
            "memblock: too many regions"
        );
        self.regions.copy_within(idx..self.cnt, idx + 1);
        self.regions[idx] = Region { base, size };
        self.cnt += 1;
        self.total_size += size;
    }

    /// `memblock_merge_regions()`: coalesces adjacent regions.
    fn merge(&mut self) {
        let mut i = 0;
        while i + 1 < self.cnt {
            let this = self.regions[i];
            let next = self.regions[i + 1];
            if this.base + this.size != next.base {
                i += 1;
                continue;
            }
            self.regions[i].size += next.size;
            self.regions.copy_within(i + 2..self.cnt, i + 1);
            self.cnt -= 1;
        }
    }

    /// `memblock_add_range()`: adds `[base, base+size)`, splitting around
    /// and merging with existing regions.
    fn add_range(&mut self, mut base: u64, size: u64) {
        if size == 0 {
            return;
        }
        let obase = base;
        let end = base + size;

        if self.cnt == 0 {
            self.regions[0] = Region { base, size };
            self.cnt = 1;
            self.total_size = size;
            return;
        }

        // Two passes: count the new regions, then insert them.
        for &insert in &[false, true] {
            base = obase;
            let mut idx = 0;
            let mut i = 0;
            while i < self.cnt {
                let rgn = self.regions[i];
                let rbase = rgn.base;
                let rend = rbase + rgn.size;
                if rbase >= end {
                    break;
                }
                if rend <= base {
                    i += 1;
                    idx += 1;
                    continue;
                }
                // The part of the new range below this region.
                if rbase > base && insert {
                    self.insert(idx, base, rbase - base);
                    idx += 1;
                }
                base = rend.min(end);
                i += 1;
                idx += 1;
            }
            // The remaining portion above the last overlapping region.
            if base < end && insert {
                self.insert(idx, base, end - base);
            }
        }
        self.merge();
    }

    /// `memblock_isolate_range()` + removal: carves `[base, end)` out.
    fn remove_range(&mut self, base: u64, size: u64) {
        let end = base + size;
        let mut i = 0;
        while i < self.cnt {
            let rgn = self.regions[i];
            let rbase = rgn.base;
            let rend = rbase + rgn.size;
            if rbase >= end || rend <= base {
                i += 1;
                continue;
            }
            let keep_lo = base.saturating_sub(rbase).min(rgn.size);
            let lo = Region {
                base: rbase,
                size: base.max(rbase) - rbase,
            };
            let hi_base = end.max(rbase);
            let hi = Region {
                base: hi_base,
                size: rend.saturating_sub(hi_base),
            };
            let _ = keep_lo;
            // Replace the region with its surviving ends.
            self.total_size -= rgn.size;
            if lo.size != 0 {
                self.regions[i] = lo;
                self.total_size += lo.size;
                i += 1;
                if hi.size != 0 {
                    self.insert(i, hi.base, hi.size);
                    i += 1;
                }
            } else if hi.size != 0 {
                self.regions[i] = hi;
                self.total_size += hi.size;
                i += 1;
            } else {
                self.regions.copy_within(i + 1..self.cnt, i);
                self.cnt -= 1;
            }
        }
    }
}

struct Memblock {
    memory: MemblockType,
    reserved: MemblockType,
}

static MEMBLOCK: SpinLock<Memblock> = SpinLock::new(Memblock {
    memory: MemblockType::new(),
    reserved: MemblockType::new(),
});

/// `memblock_add()`: records a range of usable RAM.
pub fn memblock_add(base: u64, size: u64) {
    MEMBLOCK.lock_irqsave().memory.add_range(base, size);
}

/// `memblock_reserve()`: marks a range as in use.
pub fn memblock_reserve(base: u64, size: u64) {
    MEMBLOCK.lock_irqsave().reserved.add_range(base, size);
}

/// `memblock_remove()`: drops a range from the usable map (e.g. non-RAM).
pub fn memblock_remove(base: u64, size: u64) {
    MEMBLOCK.lock_irqsave().memory.remove_range(base, size);
}

/// Calls `f(base, end)` for each page-aligned free span: RAM that is not
/// reserved (`for_each_free_mem_range()`).
pub fn for_each_free_region(mut f: impl FnMut(u64, u64)) {
    let mb = MEMBLOCK.lock_irqsave();
    for mem in &mb.memory.regions[..mb.memory.cnt] {
        let mut start = mem.base;
        let mem_end = mem.base + mem.size;
        // Walk reserved regions (kept sorted by add_range) and emit the
        // gaps between them.
        for res in &mb.reserved.regions[..mb.reserved.cnt] {
            let rbase = res.base;
            let rend = res.base + res.size;
            if rend <= start || rbase >= mem_end {
                continue;
            }
            if rbase > start {
                emit(&mut f, start, rbase.min(mem_end));
            }
            start = start.max(rend);
            if start >= mem_end {
                break;
            }
        }
        if start < mem_end {
            emit(&mut f, start, mem_end);
        }
    }
}

fn emit(f: &mut impl FnMut(u64, u64), start: u64, end: u64) {
    let s = page_align(start);
    let e = page_align_down(end);
    if s < e {
        f(s, e);
    }
}

/// `memblock_phys_mem_size()`: total usable RAM.
pub fn memory_size() -> u64 {
    MEMBLOCK.lock_irqsave().memory.total_size
}

/// `memblock_reserved_size()`.
pub fn reserved_size() -> u64 {
    MEMBLOCK.lock_irqsave().reserved.total_size
}

/// The highest usable page frame plus one (`max_pfn`).
pub fn max_pfn() -> u64 {
    let mb = MEMBLOCK.lock_irqsave();
    mb.memory.regions[..mb.memory.cnt]
        .iter()
        .map(|r| (r.base + r.size) >> 12)
        .max()
        .unwrap_or(0)
}

/// `memblock_phys_alloc()`: reserves `size` bytes (page-aligned) of free
/// RAM at or above `min_addr` and returns its physical base, or 0.
///
/// Allocates top-down from the highest free range, as Linux does by
/// default, so low memory stays available for DMA.
pub fn alloc(size: u64, align: u64, min_addr: u64) -> u64 {
    let size = page_align(size);
    let align = align.max(PAGE_SIZE);
    let mut best = 0u64;
    for_each_free_region(|start, end| {
        let start = start.max(min_addr);
        if end <= start || end - start < size {
            return;
        }
        // Highest aligned base in this span.
        let cand = (end - size) & !(align - 1);
        if cand >= start && cand > best {
            best = cand;
        }
    });
    if best != 0 {
        memblock_reserve(best, size);
    }
    best
}

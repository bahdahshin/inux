// SPDX-License-Identifier: GPL-2.0
//! A general-purpose memory-region allocator.
//!
//! A port of the bitmap core of `lib/genalloc.c` (`gen_pool_alloc` /
//! `gen_pool_free` with the default first-fit algorithm `gen_pool_first_fit`):
//! a region is divided into fixed-size allocation units tracked by a bitmap,
//! and an allocation reserves the first run of free units large enough.
//!
//! Simplified from Linux's: a single contiguous chunk (not a list grown by
//! `gen_pool_add`), a caller-supplied bitmap, first-fit only, and no locking,
//! NMI-safe `cmpxchg`, NUMA ownership or `gen_pool_dma_*` helpers.

use crate::bitmap::{bitmap_clear, bitmap_find_next_zero_area, bitmap_set};

/// A pool of memory handed out in `1 << order`-byte units.
pub struct GenPool<'a> {
    bits: &'a mut [u64],
    nunits: usize,
    start_addr: u64,
    order: u32,
    avail_units: usize,
    phys_addr: u64,
}

impl<'a> GenPool<'a> {
    /// `gen_pool_create()` + `gen_pool_add()`: a pool managing `size` bytes
    /// from `start_addr`, in units of `1 << order` bytes, tracked by `bits`.
    ///
    /// `bits` must hold at least one bit per unit (`(size >> order)` bits); it
    /// is cleared to mark the whole region free.
    pub fn new(bits: &'a mut [u64], start_addr: u64, size: usize, order: u32) -> GenPool<'a> {
        let nunits = size >> order;
        for w in bits.iter_mut() {
            *w = 0;
        }
        GenPool {
            bits,
            nunits,
            start_addr,
            order,
            avail_units: nunits,
            phys_addr: 0,
        }
    }

    #[inline]
    fn unit(&self) -> usize {
        1usize << self.order
    }

    #[inline]
    fn units_for(&self, size: usize) -> usize {
        (size + self.unit() - 1) >> self.order
    }

    /// `gen_pool_alloc()`: reserve `size` bytes, returning their start address
    /// or `None` if the pool cannot satisfy the request.
    pub fn alloc(&mut self, size: usize) -> Option<u64> {
        if size == 0 {
            return None;
        }
        let nbits = self.units_for(size);
        if nbits > self.nunits {
            return None;
        }
        let start = bitmap_find_next_zero_area(self.bits, self.nunits, 0, nbits, 0);
        if start >= self.nunits {
            return None;
        }
        bitmap_set(self.bits, start, nbits);
        self.avail_units -= nbits;
        Some(self.start_addr + ((start << self.order) as u64))
    }

    /// `gen_pool_free()`: return a `size`-byte allocation at `addr` to the
    /// pool. `addr`/`size` must match a prior [`alloc`](Self::alloc).
    pub fn free(&mut self, addr: u64, size: usize) {
        if addr < self.start_addr || size == 0 {
            return;
        }
        let start = ((addr - self.start_addr) >> self.order) as usize;
        let nbits = self.units_for(size);
        if start + nbits > self.nunits {
            return;
        }
        bitmap_clear(self.bits, start, nbits);
        self.avail_units += nbits;
    }

    /// `gen_pool_avail()`: the number of free bytes.
    pub fn avail(&self) -> usize {
        self.avail_units << self.order
    }

    /// Sets the physical address of the chunk's first byte, as the `phys`
    /// argument of `gen_pool_add_virt()` does.
    pub fn with_phys_addr(mut self, phys_addr: u64) -> Self {
        self.phys_addr = phys_addr;
        self
    }

    /// `gen_pool_virt_to_phys()`: the physical address of virtual `addr`, or
    /// `None` where C returns `-1` for an address outside the chunk.
    pub fn virt_to_phys(&self, addr: u64) -> Option<u64> {
        if self.nunits == 0 || addr < self.start_addr {
            return None;
        }
        let off = addr - self.start_addr;
        if off >= self.size() as u64 {
            return None;
        }
        Some(self.phys_addr + off)
    }

    /// `gen_pool_has_addr()`: whether `[start, start + size)` lies inside the
    /// pool. C checks the start against the chunk and the end against the
    /// chunk's last byte; a pool with no bytes has no addresses.
    pub fn has_addr(&self, start: u64, size: usize) -> bool {
        if self.nunits == 0 || size == 0 {
            return false;
        }
        let chunk_end = self.start_addr + self.size() as u64 - 1;
        let end = start.wrapping_add(size as u64 - 1);
        start >= self.start_addr && start <= chunk_end && end <= chunk_end
    }

    /// `gen_pool_size()`: the total number of bytes the pool manages.
    pub fn size(&self) -> usize {
        self.nunits << self.order
    }
}

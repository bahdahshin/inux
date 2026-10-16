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

use crate::bitmap::{
    bitmap_clear, bitmap_find_next_zero_area, bitmap_find_next_zero_area_off, bitmap_set,
};
use crate::find_bit::find_next_bit;

/// The region policy for an allocation, as `genpool_algo_t` picks it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GenAlgo {
    /// `gen_pool_first_fit()`: the first run of free units that is big enough.
    FirstFit,
    /// `gen_pool_best_fit()`: the smallest run of free units that is big enough.
    BestFit,
    /// `gen_pool_first_fit_align()`: the first run whose absolute address is a
    /// multiple of the given power-of-two alignment.
    FirstFitAlign(u64),
    /// `gen_pool_fixed_alloc()`: exactly `offset` bytes into the chunk, if that
    /// region is free. `offset` must be a whole number of units.
    Fixed(u64),
}

/// `gen_pool_best_fit()`: the start of the smallest free run of `nr` units or
/// more in `map` (`size` units), or `size` if none is big enough. A run of
/// exactly `nr` is taken at once.
fn best_fit(map: &[u64], size: usize, nr: usize) -> usize {
    let mut start_bit = size;
    let mut len = size + 1;
    let mut index = bitmap_find_next_zero_area(map, size, 0, nr, 0);

    while index < size {
        let next_bit = find_next_bit(map, size, index + nr);
        if next_bit - index < len {
            len = next_bit - index;
            start_bit = index;
            if len == nr {
                return start_bit;
            }
        }
        index = bitmap_find_next_zero_area(map, size, next_bit + 1, nr, 0);
    }
    start_bit
}

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

    /// `gen_pool_alloc()`: reserve `size` bytes with the first-fit policy,
    /// returning their start address or `None` if the pool cannot satisfy the
    /// request.
    pub fn alloc(&mut self, size: usize) -> Option<u64> {
        self.alloc_with(size, GenAlgo::FirstFit)
    }

    /// `gen_pool_alloc_algo()`: as [`alloc`](Self::alloc), but the region is
    /// chosen by `algo`.
    pub fn alloc_with(&mut self, size: usize, algo: GenAlgo) -> Option<u64> {
        if size == 0 {
            return None;
        }
        let nbits = self.units_for(size);
        if nbits > self.nunits {
            return None;
        }
        let start = match algo {
            GenAlgo::FirstFit => bitmap_find_next_zero_area(self.bits, self.nunits, 0, nbits, 0),
            GenAlgo::BestFit => best_fit(self.bits, self.nunits, nbits),
            GenAlgo::Fixed(offset) => {
                // C: an offset that is not a whole number of units is no fit.
                if offset & (self.unit() as u64 - 1) != 0 {
                    return None;
                }
                let offset_bit = (offset >> self.order) as usize;
                if offset_bit >= self.nunits {
                    return None;
                }
                let start =
                    bitmap_find_next_zero_area(self.bits, self.nunits, offset_bit, nbits, 0);
                if start != offset_bit {
                    self.nunits
                } else {
                    start
                }
            }
            GenAlgo::FirstFitAlign(align) => {
                if !align.is_power_of_two() {
                    return None;
                }
                // C: the mask in units, and the chunk start's misalignment
                // in units, so the absolute address is what gets aligned.
                let align_mask = ((align + self.unit() as u64 - 1) >> self.order) as usize - 1;
                let align_off = ((self.start_addr & (align - 1)) >> self.order) as usize;
                bitmap_find_next_zero_area_off(
                    self.bits,
                    self.nunits,
                    0,
                    nbits,
                    align_mask,
                    align_off,
                )
            }
        };
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

/// A pool made of several chunks, as `gen_pool_add()` builds with its list.
///
/// Chunks are kept in the order they were added, and searched newest first,
/// as C's `list_add_rcu()` at the head of `pool->chunks` gives. Room for at
/// most `N` chunks is fixed at compile time, since there is no allocator in
/// the crate's default build.
pub struct GenPoolList<'a, const N: usize> {
    chunks: [Option<GenPool<'a>>; N],
    len: usize,
    order: u32,
}

impl<'a, const N: usize> GenPoolList<'a, N> {
    /// `gen_pool_create()`: an empty pool whose allocations are in units of
    /// `1 << order` bytes.
    pub fn new(order: u32) -> Self {
        Self {
            chunks: core::array::from_fn(|_| None),
            len: 0,
            order,
        }
    }

    /// `gen_pool_add()`: adds a chunk of `size` bytes at `start_addr`, backed
    /// by physical `phys_addr`, tracked by `bits`. `ENOSPC` once `N` chunks
    /// are in.
    pub fn add(
        &mut self,
        bits: &'a mut [u64],
        start_addr: u64,
        size: usize,
        phys_addr: u64,
    ) -> crate::errno::Result<()> {
        if self.len == N {
            return Err(crate::errno::ENOSPC);
        }
        let chunk = GenPool::new(bits, start_addr, size, self.order).with_phys_addr(phys_addr);
        self.chunks[self.len] = Some(chunk);
        self.len += 1;
        Ok(())
    }

    /// The chunks, newest first.
    fn iter(&self) -> impl Iterator<Item = &GenPool<'a>> {
        self.chunks[..self.len].iter().rev().flatten()
    }

    /// The chunks, newest first, mutable.
    fn iter_mut(&mut self) -> impl Iterator<Item = &mut GenPool<'a>> {
        self.chunks[..self.len].iter_mut().rev().flatten()
    }

    /// `gen_pool_alloc()`: the first chunk, newest first, that can give `size`
    /// bytes.
    pub fn alloc(&mut self, size: usize) -> Option<u64> {
        self.iter_mut().find_map(|c| c.alloc(size))
    }

    /// `gen_pool_free()`: returns `size` bytes at `addr` to the chunk that
    /// contains them. An address in no chunk is ignored, as C only warns.
    pub fn free(&mut self, addr: u64, size: usize) {
        if let Some(chunk) = self.iter_mut().find(|c| c.has_addr(addr, size)) {
            chunk.free(addr, size);
        }
    }

    /// `gen_pool_avail()`: free bytes across all chunks.
    pub fn avail(&self) -> usize {
        self.iter().map(GenPool::avail).sum()
    }

    /// `gen_pool_size()`: bytes managed across all chunks.
    pub fn size(&self) -> usize {
        self.iter().map(GenPool::size).sum()
    }

    /// `gen_pool_has_addr()`: whether `[start, start + size)` lies inside one
    /// chunk.
    pub fn has_addr(&self, start: u64, size: usize) -> bool {
        self.iter().any(|c| c.has_addr(start, size))
    }

    /// `gen_pool_virt_to_phys()`: the physical address of `addr` in whichever
    /// chunk holds it.
    pub fn virt_to_phys(&self, addr: u64) -> Option<u64> {
        self.iter().find_map(|c| c.virt_to_phys(addr))
    }
}

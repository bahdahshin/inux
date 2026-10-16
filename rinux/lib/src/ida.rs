// SPDX-License-Identifier: GPL-2.0
//! ID allocator: hands out the smallest free integer id.
//!
//! Ported from the `ida_*()` part of `lib/idr.c`. The C IDA is a radix tree
//! of bitmaps that grows on demand. This version keeps the same contract,
//! that `ida_alloc_range()` returns the lowest free id in an inclusive range
//! and `ida_free()` releases one, over a fixed bitmap of `N` words, so it
//! never allocates. The IDR (pointer-mapping) half of `idr.c` is not ported.
//!
//! C's `-ENOSPC` becomes `None`. Freeing an id that is not allocated is a
//! no-op that returns `false`; C warns and does nothing.

/// An ID allocator over `64 * N` ids, numbered from zero.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ida<const N: usize> {
    bits: [u64; N],
}

impl<const N: usize> Default for Ida<N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize> Ida<N> {
    /// `ida_init()`: nothing allocated.
    pub const fn new() -> Self {
        Self { bits: [0; N] }
    }

    /// Total number of ids this allocator can hold.
    pub const fn capacity() -> usize {
        64 * N
    }

    /// Whether `id` is currently allocated.
    pub fn is_allocated(&self, id: usize) -> bool {
        id < Self::capacity() && self.bits[id / 64] & (1 << (id % 64)) != 0
    }

    /// `ida_alloc_range()`: the lowest free id in `min..=max`, marked used.
    /// Returns `None` when every id in the range is taken.
    pub fn alloc_range(&mut self, min: usize, max: usize) -> Option<usize> {
        let last = max.min(Self::capacity() - 1);
        if min > last {
            return None;
        }
        let id = (min..=last).find(|&id| !self.is_allocated(id))?;
        self.bits[id / 64] |= 1 << (id % 64);
        Some(id)
    }

    /// `ida_alloc()`: the lowest free id overall.
    pub fn alloc(&mut self) -> Option<usize> {
        self.alloc_range(0, Self::capacity() - 1)
    }

    /// `ida_free()`: release `id`. Returns `false` if it was not allocated.
    pub fn free(&mut self, id: usize) -> bool {
        if !self.is_allocated(id) {
            return false;
        }
        self.bits[id / 64] &= !(1 << (id % 64));
        true
    }
}

// SPDX-License-Identifier: GPL-2.0-only
//! Region ids for memory regions, drawn from an ID allocator.
//!
//! Ported from `lib/memregion.c`. The C file keeps one static IDA and wraps
//! `ida_alloc()` and `ida_free()`. Here the allocator is passed in, so the
//! caller owns the storage and no global is needed.

use crate::ida::Ida;

/// `memregion_alloc()`: a fresh region id, or `None` when none are left.
pub fn memregion_alloc<const N: usize>(ids: &mut Ida<N>) -> Option<usize> {
    ids.alloc()
}

/// `memregion_free()`: release a region id. Returns `false` if it was not
/// allocated.
pub fn memregion_free<const N: usize>(ids: &mut Ida<N>, id: usize) -> bool {
    ids.free(id)
}

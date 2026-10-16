// SPDX-License-Identifier: GPL-2.0
//! Memory management, as in Linux's `mm/`.

pub mod memblock;
pub mod mm_init;
pub mod mmap;
pub mod page_alloc;
pub mod rust_alloc;
pub mod slub;
pub mod vmalloc;

/// `GFP_KERNEL` and friends. rinux has one zone and no reclaim yet, so the
/// GFP flags are accepted and mostly ignored; `__GFP_ZERO` is honoured.
pub type Gfp = u32;
pub const GFP_KERNEL: Gfp = 0;
pub const GFP_ATOMIC: Gfp = 0;
/// `__GFP_ZERO`: return zeroed memory.
pub const __GFP_ZERO: Gfp = 1 << 8;

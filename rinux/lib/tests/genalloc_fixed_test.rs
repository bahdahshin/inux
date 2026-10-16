// SPDX-License-Identifier: GPL-2.0
//! Host tests for the fixed-offset policy in `lib/genalloc.c`
//! (`gen_pool_fixed_alloc()`): the region is taken only at the given offset.

use rinux_lib::genalloc::{GenAlgo, GenPool};

#[test]
fn a_fixed_region_is_taken_at_its_offset() {
    let mut bits = [0u64; 1];
    let mut pool = GenPool::new(&mut bits, 0x1000, 32, 0);
    assert_eq!(pool.alloc_with(2, GenAlgo::Fixed(4)), Some(0x1004));
}

#[test]
fn a_fixed_region_that_is_taken_is_refused() {
    let mut bits = [0u64; 1];
    let mut pool = GenPool::new(&mut bits, 0x1000, 32, 0);
    assert_eq!(pool.alloc_with(2, GenAlgo::Fixed(4)), Some(0x1004));
    assert_eq!(pool.alloc_with(1, GenAlgo::Fixed(5)), None);
}

#[test]
fn an_offset_that_is_not_a_whole_unit_is_refused() {
    let mut bits = [0u64; 1];
    // Four-byte units, so an offset of 6 is not a unit boundary.
    let mut pool = GenPool::new(&mut bits, 0x1000, 64, 2);
    assert_eq!(pool.alloc_with(4, GenAlgo::Fixed(6)), None);
    assert_eq!(pool.alloc_with(4, GenAlgo::Fixed(8)), Some(0x1008));
}

#[test]
fn an_offset_past_the_chunk_is_refused() {
    let mut bits = [0u64; 1];
    let mut pool = GenPool::new(&mut bits, 0x1000, 32, 0);
    assert_eq!(pool.alloc_with(1, GenAlgo::Fixed(32)), None);
}

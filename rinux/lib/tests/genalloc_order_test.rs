// SPDX-License-Identifier: GPL-2.0
//! Host tests for the order-aligned policy in `lib/genalloc.c`
//! (`gen_pool_first_fit_order_align()`): a request of `nr` units starts at a
//! multiple of `nr` rounded up to a power of two, by unit index.

use rinux_lib::genalloc::{GenAlgo, GenPool};

#[test]
fn a_three_unit_request_starts_at_a_multiple_of_four() {
    let mut bits = [0u64; 1];
    let mut pool = GenPool::new(&mut bits, 0x1000, 32, 0);
    assert_eq!(pool.alloc(1), Some(0x1000)); // unit 0 used
    // Three units round up to four, so the next aligned start is unit 4.
    assert_eq!(pool.alloc_with(3, GenAlgo::OrderAlign), Some(0x1004));
}

#[test]
fn a_one_unit_request_is_not_aligned() {
    let mut bits = [0u64; 1];
    let mut pool = GenPool::new(&mut bits, 0x1000, 32, 0);
    assert_eq!(pool.alloc(1), Some(0x1000));
    assert_eq!(pool.alloc_with(1, GenAlgo::OrderAlign), Some(0x1001));
}

#[test]
fn an_aligned_run_that_does_not_fit_is_skipped() {
    let mut bits = [0u64; 1];
    let mut pool = GenPool::new(&mut bits, 0x1000, 8, 0);
    assert_eq!(pool.alloc(3), Some(0x1000)); // units 0..3
    // Four units aligned to four: unit 4 is the only candidate, and it fits.
    assert_eq!(pool.alloc_with(4, GenAlgo::OrderAlign), Some(0x1004));
    // The next four would need unit 8, which is past the 8-unit chunk.
    assert_eq!(pool.alloc_with(4, GenAlgo::OrderAlign), None);
}

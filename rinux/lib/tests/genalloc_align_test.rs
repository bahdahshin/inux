// SPDX-License-Identifier: GPL-2.0
//! Host tests for the aligned policy in `lib/genalloc.c`
//! (`gen_pool_first_fit_align()`): the absolute address, not the offset in the
//! pool, is what is aligned.

use rinux_lib::genalloc::{GenAlgo, GenPool};

#[test]
fn an_aligned_start_skips_to_the_next_aligned_address() {
    let mut bits = [0u64; 1];
    let mut pool = GenPool::new(&mut bits, 0x1000, 32, 0);
    assert_eq!(pool.alloc(1), Some(0x1000));
    // The next 8-aligned address after the used byte is 0x1008.
    assert_eq!(pool.alloc_with(2, GenAlgo::FirstFitAlign(8)), Some(0x1008));
}

#[test]
fn an_unaligned_pool_start_is_taken_into_account() {
    let mut bits = [0u64; 1];
    // The pool starts at 0x1003, so the first 8-aligned address is 0x1008,
    // which is five units in.
    let mut pool = GenPool::new(&mut bits, 0x1003, 32, 0);
    assert_eq!(pool.alloc_with(1, GenAlgo::FirstFitAlign(8)), Some(0x1008));
}

#[test]
fn a_non_power_of_two_alignment_is_refused() {
    let mut bits = [0u64; 1];
    let mut pool = GenPool::new(&mut bits, 0x1000, 32, 0);
    assert_eq!(pool.alloc_with(1, GenAlgo::FirstFitAlign(6)), None);
}

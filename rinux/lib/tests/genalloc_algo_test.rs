// SPDX-License-Identifier: GPL-2.0
//! Host tests for the allocation policies in `lib/genalloc.c`: first-fit takes
//! the first free run that fits, best-fit takes the smallest one.

use rinux_lib::genalloc::{GenAlgo, GenPool};

/// Lays out 16 one-byte units as: free run 0..4 (len 4), used 4, free run
/// 5..7 (len 2), used 7..16.
fn fragmented(bits: &mut [u64; 1]) -> GenPool<'_> {
    let mut pool = GenPool::new(bits, 0, 16, 0);
    assert_eq!(pool.alloc(4), Some(0)); // 0..4
    assert_eq!(pool.alloc(1), Some(4)); // 4
    assert_eq!(pool.alloc(2), Some(5)); // 5..7
    assert_eq!(pool.alloc(9), Some(7)); // 7..16
    pool.free(0, 4);
    pool.free(5, 2);
    pool
}

#[test]
fn first_fit_takes_the_first_run_that_fits() {
    let mut bits = [0u64; 1];
    let mut pool = fragmented(&mut bits);
    assert_eq!(pool.alloc_with(2, GenAlgo::FirstFit), Some(0));
}

#[test]
fn best_fit_takes_the_smallest_run_that_fits() {
    let mut bits = [0u64; 1];
    let mut pool = fragmented(&mut bits);
    assert_eq!(pool.alloc_with(2, GenAlgo::BestFit), Some(5));
}

#[test]
fn best_fit_gives_none_when_no_run_is_big_enough() {
    let mut bits = [0u64; 1];
    let mut pool = fragmented(&mut bits);
    assert_eq!(pool.alloc_with(5, GenAlgo::BestFit), None);
}

#[test]
fn plain_alloc_is_first_fit() {
    let mut bits = [0u64; 1];
    let mut pool = fragmented(&mut bits);
    assert_eq!(pool.alloc(2), Some(0));
}

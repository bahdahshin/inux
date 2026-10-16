// SPDX-License-Identifier: GPL-2.0
//! Host tests for `gen_pool_has_addr()` (`lib/genalloc.c`) on a single-chunk
//! pool: the start must be inside the chunk, and the range must end inside it.

use rinux_lib::genalloc::GenPool;

#[test]
fn ranges_inside_the_chunk_are_found() {
    let mut bits = [0u64; 4];
    // A 256-byte chunk at 0x1000 with one-byte units: last byte 0x10ff.
    let pool = GenPool::new(&mut bits, 0x1000, 256, 0);
    assert!(pool.has_addr(0x1000, 1));
    assert!(pool.has_addr(0x1000, 256));
    assert!(pool.has_addr(0x10ff, 1));
}

#[test]
fn ranges_that_leave_the_chunk_are_not_found() {
    let mut bits = [0u64; 4];
    let pool = GenPool::new(&mut bits, 0x1000, 256, 0);
    assert!(!pool.has_addr(0x0fff, 2), "starts before the chunk");
    assert!(!pool.has_addr(0x10ff, 2), "ends after the chunk");
    assert!(!pool.has_addr(0x1100, 1), "starts after the chunk");
}

#[test]
fn an_empty_range_or_pool_is_not_found() {
    let mut bits = [0u64; 4];
    let pool = GenPool::new(&mut bits, 0x1000, 256, 0);
    assert!(!pool.has_addr(0x1000, 0));
    let mut none: [u64; 0] = [];
    let empty = GenPool::new(&mut none, 0x1000, 0, 0);
    assert!(!empty.has_addr(0x1000, 1));
}

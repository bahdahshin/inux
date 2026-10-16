// SPDX-License-Identifier: GPL-2.0
//! Host tests for the growable pool (`gen_pool_add()` with no fixed chunk
//! count): adding past any limit, newest-first allocation, and free to the
//! right chunk. Builds only with the `alloc` feature.
#![cfg(feature = "alloc")]

use rinux_lib::genalloc::{GenAlgo, GenPoolVec};

#[test]
fn chunks_keep_being_added_and_allocation_is_newest_first() {
    let mut a = [0u64; 4];
    let mut b = [0u64; 2];
    let mut c = [0u64; 1];
    let mut pool = GenPoolVec::new(0);
    pool.add(&mut a, 0x1000, 256, 0x8000);
    pool.add(&mut b, 0x2000, 128, 0x9000);
    pool.add(&mut c, 0x3000, 64, 0xa000);

    assert_eq!(pool.size(), 448);
    // The newest chunk, 64 bytes, is tried first.
    assert_eq!(pool.alloc(50), Some(0x3000));
    // 100 bytes do not fit the 14 left in it, so the 128-byte chunk serves them.
    assert_eq!(pool.alloc(100), Some(0x2000));
    assert_eq!(pool.avail(), 448 - 50 - 100);
}

#[test]
fn free_and_translation_work_across_all_chunks() {
    let mut a = [0u64; 4];
    let mut b = [0u64; 2];
    let mut pool = GenPoolVec::new(0);
    pool.add(&mut a, 0x1000, 256, 0x8000);
    pool.add(&mut b, 0x2000, 128, 0x9000);

    let x = pool.alloc(100).unwrap();
    let before = pool.avail();
    pool.free(x, 100);
    assert_eq!(pool.avail(), before + 100);
    assert_eq!(pool.virt_to_phys(0x2010), Some(0x9010));
    assert!(pool.has_addr(0x2000, 128));
    assert!(!pool.has_addr(0x10f0, 0x20));
}

#[test]
fn the_policies_apply_across_chunks() {
    let mut a = [0u64; 4];
    let mut b = [0u64; 2];
    let mut pool = GenPoolVec::new(0);
    pool.add(&mut a, 0x1000, 256, 0x8000);
    pool.add(&mut b, 0x2000, 128, 0x9000);
    assert_eq!(pool.alloc_with(4, GenAlgo::Fixed(8)), Some(0x2008));
}

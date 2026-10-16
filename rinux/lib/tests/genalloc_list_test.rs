// SPDX-License-Identifier: GPL-2.0
//! Host tests for the multi-chunk pool (`gen_pool_add()` and the list walk in
//! `lib/genalloc.c`): newest chunk first, free to the chunk that holds the
//! range, and physical addresses per chunk.

use rinux_lib::errno::ENOSPC;
use rinux_lib::genalloc::GenPoolList;

#[test]
fn allocation_tries_the_newest_chunk_first_and_then_the_older_ones() {
    let mut a = [0u64; 4];
    let mut b = [0u64; 2];
    let mut pool: GenPoolList<'_, 2> = GenPoolList::new(0);
    pool.add(&mut a, 0x1000, 256, 0x8000).unwrap();
    pool.add(&mut b, 0x2000, 128, 0x9000).unwrap();

    assert_eq!(pool.size(), 384);
    assert_eq!(pool.avail(), 384);
    // 200 bytes do not fit the newer 128-byte chunk, so the older one serves it.
    assert_eq!(pool.alloc(200), Some(0x1000));
    // 100 bytes fit the newer chunk, which is tried first.
    assert_eq!(pool.alloc(100), Some(0x2000));
    assert_eq!(pool.avail(), 384 - 200 - 100);
}

#[test]
fn free_returns_the_bytes_to_the_chunk_that_holds_them() {
    let mut a = [0u64; 4];
    let mut b = [0u64; 2];
    let mut pool: GenPoolList<'_, 2> = GenPoolList::new(0);
    pool.add(&mut a, 0x1000, 256, 0x8000).unwrap();
    pool.add(&mut b, 0x2000, 128, 0x9000).unwrap();

    let x = pool.alloc(200).unwrap();
    let before = pool.avail();
    pool.free(x, 200);
    assert_eq!(pool.avail(), before + 200);
    // The freed space is usable again.
    assert_eq!(pool.alloc(200), Some(0x1000));
}

#[test]
fn addresses_map_to_the_physical_base_of_their_chunk() {
    let mut a = [0u64; 4];
    let mut b = [0u64; 2];
    let mut pool: GenPoolList<'_, 2> = GenPoolList::new(0);
    pool.add(&mut a, 0x1000, 256, 0x8000).unwrap();
    pool.add(&mut b, 0x2000, 128, 0x9000).unwrap();

    assert_eq!(pool.virt_to_phys(0x1010), Some(0x8010));
    assert_eq!(pool.virt_to_phys(0x2010), Some(0x9010));
    assert_eq!(pool.virt_to_phys(0x3000), None);
}

#[test]
fn a_range_must_lie_inside_one_chunk() {
    let mut a = [0u64; 4];
    let mut b = [0u64; 2];
    let mut pool: GenPoolList<'_, 2> = GenPoolList::new(0);
    pool.add(&mut a, 0x1000, 256, 0x8000).unwrap();
    pool.add(&mut b, 0x2000, 128, 0x9000).unwrap();

    assert!(pool.has_addr(0x2000, 128));
    assert!(pool.has_addr(0x1000, 256));
    // The range starts in the first chunk and runs past its end.
    assert!(!pool.has_addr(0x10f0, 0x20));
}

#[test]
fn adding_past_the_chunk_slots_is_enospc() {
    let mut a = [0u64; 4];
    let mut b = [0u64; 2];
    let mut pool: GenPoolList<'_, 1> = GenPoolList::new(0);
    pool.add(&mut a, 0x1000, 256, 0x8000).unwrap();
    assert_eq!(pool.add(&mut b, 0x2000, 128, 0x9000), Err(ENOSPC));
}

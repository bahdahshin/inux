// SPDX-License-Identifier: GPL-2.0
//! Tests for the general-purpose pool allocator.

use rinux_lib::genalloc::GenPool;

const BASE: u64 = 0x1_0000;

#[test]
fn alloc_free_reuse() {
    // 512 bytes in 8-byte units (order 3) => 64 units => 1 bitmap word.
    let mut bits = [0u64; 1];
    let mut pool = GenPool::new(&mut bits, BASE, 512, 3);
    assert_eq!(pool.size(), 512);
    assert_eq!(pool.avail(), 512);

    let a = pool.alloc(8).unwrap(); // 1 unit
    let b = pool.alloc(16).unwrap(); // 2 units
    let c = pool.alloc(8).unwrap(); // 1 unit
    assert_eq!(a, BASE);
    assert_eq!(b, BASE + 8);
    assert_eq!(c, BASE + 24);
    assert_eq!(pool.avail(), 512 - 32);

    // Freeing the first block lets first-fit reuse it.
    pool.free(a, 8);
    assert_eq!(pool.avail(), 512 - 24);
    assert_eq!(pool.alloc(8).unwrap(), BASE);
}

#[test]
fn exhaustion() {
    // 8 units of 16 bytes.
    let mut bits = [0u64; 1];
    let mut pool = GenPool::new(&mut bits, BASE, 128, 4);
    let mut addrs = Vec::new();
    for _ in 0..8 {
        addrs.push(pool.alloc(16).unwrap());
    }
    assert_eq!(pool.avail(), 0);
    assert!(pool.alloc(16).is_none()); // full
    assert!(pool.alloc(1).is_none());

    // Free one; a 16-byte request fits again, a 32-byte one does not.
    pool.free(addrs[3], 16);
    assert!(pool.alloc(32).is_none());
    assert_eq!(pool.alloc(16).unwrap(), addrs[3]);
}

#[test]
fn no_overlap() {
    // 256 units of 8 bytes (4 bitmap words).
    let mut bits = [0u64; 4];
    let mut pool = GenPool::new(&mut bits, BASE, 2048, 3);
    // Allocate a mix of sizes and record the byte ranges.
    let sizes = [8usize, 24, 16, 8, 40, 32, 8, 64, 16, 8];
    let mut ranges: Vec<(u64, u64)> = Vec::new();
    for &s in &sizes {
        let a = pool.alloc(s).unwrap();
        let end = a + ((s + 7) & !7) as u64; // rounded to the unit
        for &(lo, hi) in &ranges {
            assert!(end <= lo || a >= hi, "overlap: [{a},{end}) vs [{lo},{hi})");
        }
        assert!(a >= BASE && end <= BASE + 2048);
        ranges.push((a, end));
    }
}

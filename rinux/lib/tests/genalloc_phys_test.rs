// SPDX-License-Identifier: GPL-2.0
//! Host tests for `gen_pool_virt_to_phys()` (`lib/genalloc.c`) on a
//! single-chunk pool: the offset into the chunk is added to its physical base,
//! and an address outside the chunk has no physical address.

use rinux_lib::genalloc::GenPool;

#[test]
fn an_address_maps_to_the_chunk_physical_base_plus_its_offset() {
    let mut bits = [0u64; 4];
    let pool = GenPool::new(&mut bits, 0x1000, 256, 0).with_phys_addr(0x8000_0000);
    assert_eq!(pool.virt_to_phys(0x1000), Some(0x8000_0000));
    assert_eq!(pool.virt_to_phys(0x1010), Some(0x8000_0010));
    assert_eq!(pool.virt_to_phys(0x10ff), Some(0x8000_00ff));
}

#[test]
fn an_address_outside_the_chunk_has_no_physical_address() {
    let mut bits = [0u64; 4];
    let pool = GenPool::new(&mut bits, 0x1000, 256, 0).with_phys_addr(0x8000_0000);
    assert_eq!(pool.virt_to_phys(0x0fff), None);
    assert_eq!(pool.virt_to_phys(0x1100), None);
}

#[test]
fn the_default_physical_base_is_zero() {
    let mut bits = [0u64; 4];
    let pool = GenPool::new(&mut bits, 0x1000, 256, 0);
    assert_eq!(pool.virt_to_phys(0x1020), Some(0x20));
}

// SPDX-License-Identifier: GPL-2.0-or-later
//! Host tests for the `ida` allocator and `memregion`. Linux has no KUnit
//! file for either.

use rinux_lib::ida::Ida;
use rinux_lib::memregion::{memregion_alloc, memregion_free};

#[test]
fn ids_come_out_lowest_first_and_are_reused() {
    let mut ida = Ida::<1>::new();
    assert_eq!(ida.alloc(), Some(0));
    assert_eq!(ida.alloc(), Some(1));
    assert_eq!(ida.alloc(), Some(2));
    assert!(ida.free(1));
    assert_eq!(ida.alloc(), Some(1));
}

#[test]
fn a_range_is_inclusive_and_respects_its_bounds() {
    let mut ida = Ida::<1>::new();
    assert_eq!(ida.alloc_range(10, 12), Some(10));
    assert_eq!(ida.alloc_range(10, 12), Some(11));
    assert_eq!(ida.alloc_range(10, 11), None);
    assert_eq!(ida.alloc_range(12, 12), Some(12));
    assert_eq!(ida.alloc_range(12, 12), None);
    assert_eq!(ida.alloc_range(5, 3), None);
}

#[test]
fn a_full_allocator_refuses_and_freeing_an_unused_id_is_a_no_op() {
    let mut ida = Ida::<1>::new();
    for _ in 0..Ida::<1>::capacity() {
        assert!(ida.alloc().is_some());
    }
    assert_eq!(ida.alloc(), None);
    assert!(ida.free(63));
    assert!(!ida.free(63));
    assert!(!ida.free(1000));
}

#[test]
fn memregion_ids_follow_the_allocator() {
    let mut ids = Ida::<1>::new();
    let a = memregion_alloc(&mut ids).unwrap();
    let b = memregion_alloc(&mut ids).unwrap();
    assert_ne!(a, b);
    assert!(memregion_free(&mut ids, a));
    assert_eq!(memregion_alloc(&mut ids), Some(a));
}

// SPDX-License-Identifier: GPL-2.0+
//! Host port of the parts of `lib/test_ida.c` that fit the fixed-size `Ida`.
//!
//! Ported: the straightforward allocation checks, the leaf fill-and-delete,
//! the exceptional-entry conversion, the bad-free no-ops, and the
//! existence checks behind `ida_find_first_range()`.
//!
//! Not ported: `ida_check_max()` needs ids near 2^31, and `ida_destroy()`,
//! `ida_is_empty()` and `ida_find_first_range()` have no counterpart in the
//! fixed bitmap yet. Those checks are left for when the `Ida` grows a tree.

use rinux_lib::ida::Ida;

/// `IDA_BITMAP_BITS` in the C code.
const BITMAP_BITS: usize = 1024;

type TestIda = Ida<256>; // 16384 ids, far above the largest id used below

/// `ida_check_alloc()`.
#[test]
fn check_alloc() {
    let mut ida = TestIda::new();
    for i in 0..10000 {
        assert_eq!(ida.alloc(), Some(i));
    }
    assert!(ida.free(20));
    assert!(ida.free(21));
    for i in 0..3 {
        let id = ida.alloc().expect("id");
        if i == 2 {
            assert_eq!(id, 10000);
        }
    }
    for i in 0..5000 {
        assert!(ida.free(i));
    }
    assert_eq!(ida.alloc_range(5000, TestIda::capacity() - 1), Some(10001));
}

/// `ida_check_leaf()`: fill one bitmap leaf, then free it.
#[test]
fn check_leaf() {
    for base in [0, 1024, 1024 * 4] {
        let mut ida = TestIda::new();
        for i in 0..BITMAP_BITS {
            assert_eq!(
                ida.alloc_range(base, TestIda::capacity() - 1),
                Some(base + i)
            );
        }
        for i in 0..BITMAP_BITS {
            assert!(ida.free(base + i));
        }
        assert_eq!(ida.alloc(), Some(0));
        assert!(ida.free(0));
    }
}

/// `ida_check_conv()`: alternate single ids and fills across leaf edges.
#[test]
fn check_conv() {
    let mut ida = TestIda::new();
    for i in (0..BITMAP_BITS * 2).step_by(BITMAP_BITS) {
        assert_eq!(ida.alloc_range(i + 1, usize::MAX), Some(i + 1));
        assert_eq!(ida.alloc_range(i + 64, usize::MAX), Some(i + 64));
        assert!(ida.free(i + 1));
        assert!(ida.free(i + 64));
        assert!(!ida.is_allocated(i + 1));
        assert!(!ida.is_allocated(i + 64));
    }
    for i in 0..BITMAP_BITS * 2 {
        assert_eq!(ida.alloc(), Some(i));
    }
    for i in (0..BITMAP_BITS * 2).rev() {
        assert!(ida.free(i));
    }
    for i in 0..BITMAP_BITS + 60 {
        assert_eq!(ida.alloc(), Some(i));
    }
    for i in (0..BITMAP_BITS + 60).rev() {
        assert!(ida.free(i));
    }
    assert!(!ida.is_allocated(0));
}

/// `ida_check_bad_free()`: freeing ids the IDA does not hold does nothing.
#[test]
fn check_bad_free() {
    let mut ida = TestIda::new();
    assert!(!ida.free(0));
    for i in 0..31 {
        assert!(!ida.free(1 << i));
    }
    assert_eq!(ida.alloc_range(3, usize::MAX), Some(3));
    assert!(!ida.free(0));
    for i in 0..31 {
        assert!(!ida.free(1 << i));
    }
    assert!(ida.free(3));
    assert!(!ida.free(3));
}

/// The existence checks from `ida_check_find_first()`, minus the calls the
/// fixed bitmap cannot express. Ids 3, 63, 1023 and 2^20-1 are used.
#[test]
fn check_exists_and_find_first() {
    let mut ida = Ida::<{ (1 << 20) / 64 }>::new();
    assert!(!ida.is_allocated(3));
    assert_eq!(ida.alloc_range(3, usize::MAX), Some(3));
    assert!(!ida.is_allocated(0));
    assert!(ida.is_allocated(3));
    assert!(!ida.is_allocated(63));
    assert_eq!(ida.alloc_range(63, usize::MAX), Some(63));
    assert_eq!(ida.alloc_range(1023, usize::MAX), Some(1023));
    assert_eq!(
        ida.alloc_range((1 << 20) - 1, usize::MAX),
        Some((1 << 20) - 1)
    );
    assert_eq!(ida.alloc_range(64, 1026), Some(64));
    assert!(ida.free(64));
    assert_eq!(ida.alloc_range(1024, (1 << 20) - 1), Some(1024));
    assert!(ida.free(1024));
    assert_eq!(ida.alloc_range(1 << 20, usize::MAX), None);
    assert!(ida.free(3));
    assert!(ida.free(63));
    assert!(ida.free(1023));
    assert!(ida.free((1 << 20) - 1));
}

// SPDX-License-Identifier: GPL-2.0-or-later
//! Host tests for `iommu-helper`. Linux has no KUnit file for it; these
//! cover allocation, boundary skipping and exhaustion.

use rinux_lib::bitmap::bitmap_set;
use rinux_lib::iommu_helper::{iommu_area_alloc, iommu_device_max_index, iommu_is_span_boundary};

#[test]
fn allocations_are_sequential_and_mark_bits() {
    let mut map = [0u64; 1];
    assert_eq!(iommu_area_alloc(&mut map, 64, 0, 8, 0, 64, 0), Some(0));
    assert_eq!(iommu_area_alloc(&mut map, 64, 0, 8, 0, 64, 0), Some(8));
    assert_eq!(map[0] & 0xffff, 0xffff);
}

#[test]
fn an_allocation_that_would_cross_a_boundary_moves_to_the_next_one() {
    let mut map = [0u64; 1];
    bitmap_set(&mut map, 0, 8);
    // 12 bits from index 8 would run from 8 to 20, across the 16 boundary.
    assert_eq!(iommu_area_alloc(&mut map, 64, 8, 12, 0, 16, 0), Some(16));
}

#[test]
fn a_full_area_returns_none() {
    let mut map = [u64::MAX; 1];
    assert_eq!(iommu_area_alloc(&mut map, 64, 0, 1, 0, 64, 0), None);

    let mut map = [0u64; 1];
    // Only 15 usable bits: the last of the limit is never handed out.
    assert_eq!(iommu_area_alloc(&mut map, 16, 0, 16, 0, 64, 0), None);
}

#[test]
fn span_boundary_checks_the_shifted_offset() {
    assert!(!iommu_is_span_boundary(0, 16, 0, 16));
    assert!(iommu_is_span_boundary(1, 16, 0, 16));
    // A shift moves the start within the boundary.
    assert!(iommu_is_span_boundary(0, 4, 14, 16));
    assert!(!iommu_is_span_boundary(0, 2, 14, 16));
}

#[test]
fn device_max_index_is_clamped_by_the_dma_mask() {
    assert_eq!(iommu_device_max_index(100, 0, 50), 51);
    assert_eq!(iommu_device_max_index(40, 0, 50), 40);
}

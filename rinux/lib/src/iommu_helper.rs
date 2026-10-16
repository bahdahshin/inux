// SPDX-License-Identifier: GPL-2.0
//! Free-area allocation for IOMMU and DMA address space.
//!
//! Ported from `lib/iommu-helper.c` and `include/linux/iommu-helper.h`. A
//! bitmap tracks which pages of an address range are in use; allocations
//! must not straddle a power-of-two boundary, such as a 4 GiB segment.
//!
//! C's `-1` return becomes `None`. The `BUG_ON()` on a non-power-of-two
//! boundary becomes an assertion.

use crate::bitmap::{bitmap_find_next_zero_area, bitmap_set};

/// `iommu_device_max_index()`: how many indices a device can address,
/// given its DMA mask and the offset of the range.
pub fn iommu_device_max_index(size: u64, offset: u64, dma_mask: u64) -> u64 {
    if size + offset > dma_mask {
        dma_mask - offset + 1
    } else {
        size
    }
}

/// `iommu_is_span_boundary()`: would `nr` indices starting at `index`,
/// shifted by `shift`, cross a `boundary_size` boundary?
pub fn iommu_is_span_boundary(index: u64, nr: u64, shift: u64, boundary_size: u64) -> bool {
    assert!(
        boundary_size.is_power_of_two(),
        "boundary_size must be a power of two"
    );
    let offset = (shift + index) & (boundary_size - 1);
    offset + nr > boundary_size
}

/// `iommu_area_alloc()`: find `nr` clear bits in `map` at or after `start`,
/// avoiding boundary crossings, and mark them used. `size` is the number of
/// valid bits and `align_mask` is the usual `bitmap_find_next_zero_area()`
/// alignment mask.
///
/// Returns the first index, or `None` when the area is full.
pub fn iommu_area_alloc(
    map: &mut [u64],
    size: usize,
    mut start: usize,
    nr: usize,
    shift: usize,
    boundary_size: usize,
    align_mask: usize,
) -> Option<usize> {
    // We don't want the last of the limit.
    let size = size - 1;
    loop {
        let index = bitmap_find_next_zero_area(map, size, start, nr, align_mask);
        if index >= size {
            return None;
        }
        if span_boundary(index, nr, shift, boundary_size) {
            // ALIGN(shift + index, boundary_size) - shift
            start = (shift + index).next_multiple_of(boundary_size) - shift;
            continue;
        }
        bitmap_set(map, index, nr);
        return Some(index);
    }
}

/// The `usize` form of [`iommu_is_span_boundary`] used by the allocator.
fn span_boundary(index: usize, nr: usize, shift: usize, boundary_size: usize) -> bool {
    iommu_is_span_boundary(index as u64, nr as u64, shift as u64, boundary_size as u64)
}

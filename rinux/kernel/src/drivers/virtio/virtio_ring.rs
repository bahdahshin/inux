// SPDX-License-Identifier: GPL-2.0
//! The split virtqueue.
//!
//! A small port of `drivers/virtio/virtio_ring.c`: lay out a split ring in
//! physically contiguous memory, expose a buffer to the device, and collect
//! completions. The ring pages come from the buddy allocator and are
//! identity mapped, so a physical address doubles as the pointer the driver
//! dereferences and as the address it hands the device for DMA.
//!
//! Simplified from Linux's: no indirect descriptors, no event index, no
//! DMA-API bounce buffering, and one request in flight at a time (descriptor
//! 0), so the free-list bookkeeping of `virtqueue_add()` is unnecessary.

use core::sync::atomic::{Ordering, fence};

use crate::arch::x86::include::page::PAGE_SIZE;
use crate::include::uapi::virtio_ring::*;
use crate::mm::__GFP_ZERO;
use crate::mm::page_alloc::{alloc_pages, free_pages};

/// A split virtqueue backed by one buddy allocation.
pub struct Vring {
    /// Physical (== virtual) base of the ring region.
    base: u64,
    num: usize,
    order: u32,
    /// Byte offset of `vring_avail` within the region.
    avail_off: usize,
    /// Byte offset of `vring_used` within the region.
    used_off: usize,
}

impl Vring {
    /// Allocates and zeroes a ring of `num` descriptors.
    pub fn new(num: usize) -> Option<Vring> {
        let size = vring_size(num, PAGE_SIZE as usize);
        let mut order = 0u32;
        while (PAGE_SIZE as usize) << order < size {
            order += 1;
        }
        let base = alloc_pages(__GFP_ZERO, order);
        if base == 0 {
            return None;
        }
        // desc: [0, 16*num); avail follows; used starts on the next page.
        let avail_off = 16 * num;
        let desc_avail = avail_off + 2 * (3 + num);
        let used_off = desc_avail.next_multiple_of(PAGE_SIZE as usize);
        Some(Vring {
            base,
            num,
            order,
            avail_off,
            used_off,
        })
    }

    /// The page frame number the transport programs into `QUEUE_PFN`.
    pub fn pfn(&self) -> u32 {
        (self.base >> 12) as u32
    }

    pub fn num(&self) -> usize {
        self.num
    }

    /// Writes descriptor `idx`.
    pub fn set_desc(&self, idx: usize, addr: u64, len: u32, flags: u16, next: u16) {
        let d = (self.base + (idx * 16) as u64) as *mut VringDesc;
        // SAFETY: idx < num, so d is within the descriptor table.
        unsafe {
            d.write_volatile(VringDesc {
                addr,
                len,
                flags,
                next,
            })
        };
    }

    #[inline]
    fn avail_idx_ptr(&self) -> *mut u16 {
        (self.base + self.avail_off as u64 + 2) as *mut u16
    }

    /// Publishes descriptor chain `head` to the device: place it in the
    /// available ring, then advance the index after a write barrier so the
    /// device never sees a stale ring entry under a fresh index.
    pub fn publish(&self, head: u16) {
        // SAFETY: the avail ring and index live inside the ring region.
        unsafe {
            let idx = self.avail_idx_ptr().read_volatile();
            let slot = (idx as usize) % self.num;
            let ring = (self.base + self.avail_off as u64 + 4 + (slot * 2) as u64) as *mut u16;
            ring.write_volatile(head);
            // Ensure the ring entry is visible before the index that exposes it.
            fence(Ordering::Release);
            self.avail_idx_ptr().write_volatile(idx.wrapping_add(1));
        }
    }

    /// The device's used-ring index (how many chains it has completed).
    pub fn used_idx(&self) -> u16 {
        // SAFETY: used.idx is the 16-bit word after used.flags.
        let v = unsafe { ((self.base + self.used_off as u64 + 2) as *const u16).read_volatile() };
        fence(Ordering::Acquire);
        v
    }

    /// Reads used-ring element `slot` (`id`, `len`).
    pub fn used_elem(&self, slot: usize) -> VringUsedElem {
        let p = (self.base + self.used_off as u64 + 4 + (slot * 8) as u64) as *const VringUsedElem;
        // SAFETY: slot < num, within the used ring.
        unsafe { p.read_volatile() }
    }
}

impl Drop for Vring {
    fn drop(&mut self) {
        if self.base != 0 {
            free_pages(self.base, self.order);
        }
    }
}

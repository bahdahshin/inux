// SPDX-License-Identifier: GPL-2.0 WITH Linux-syscall-note
//! The virtio split virtqueue layout.
//!
//! A subset of `include/uapi/linux/virtio_ring.h`: the descriptor flags and
//! the `vring_size()` layout for a split ring. The packed-ring and
//! event-index extensions are not ported; the driver uses a plain split
//! ring with notifications on every request.

/// This marks a buffer as continuing via the `next` field.
pub const VRING_DESC_F_NEXT: u16 = 1;
/// This marks a buffer as device write-only (otherwise device read-only).
pub const VRING_DESC_F_WRITE: u16 = 2;

/// The device uses this in `used->flags` to tell the driver not to kick.
pub const VRING_USED_F_NO_NOTIFY: u16 = 1;
/// The driver uses this in `avail->flags` to ask not to be interrupted.
pub const VRING_AVAIL_F_NO_INTERRUPT: u16 = 1;

/// Alignment of the used ring (`VRING_USED_ALIGN_SIZE`).
pub const VRING_USED_ALIGN_SIZE: usize = 4;

/// `struct vring_desc`: one scatter-gather entry. 16 bytes.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct VringDesc {
    /// Guest-physical address of the buffer.
    pub addr: u64,
    /// Length of the buffer.
    pub len: u32,
    /// `VRING_DESC_F_*`.
    pub flags: u16,
    /// Index of the next chained descriptor, if `VRING_DESC_F_NEXT`.
    pub next: u16,
}

/// `struct vring_used_elem`: a completed descriptor chain. 8 bytes.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct VringUsedElem {
    /// Index of the head descriptor of the chain that completed.
    pub id: u32,
    /// Total number of bytes the device wrote into the chain.
    pub len: u32,
}

/// `vring_size(num, align)`: bytes needed for a split ring of `num`
/// descriptors, with the used ring aligned to `align`.
///
/// Layout: the descriptor table (`16 * num`) and the available ring
/// (`2 * (3 + num)`) packed together and rounded up to `align`, then the
/// used ring (`2 * 3 + 8 * num`).
pub const fn vring_size(num: usize, align: usize) -> usize {
    let desc_avail = 16 * num + 2 * (3 + num);
    let rounded = (desc_avail + align - 1) & !(align - 1);
    rounded + 2 * 3 + 8 * num
}

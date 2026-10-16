// SPDX-License-Identifier: GPL-2.0 WITH Linux-syscall-note
//! Virtio device type IDs.
//!
//! A subset of `include/uapi/linux/virtio_ids.h`. For a legacy or
//! transitional virtio-pci device the type also appears as the PCI
//! subsystem ID.

pub const VIRTIO_ID_NET: u16 = 1; // virtio net
pub const VIRTIO_ID_BLOCK: u16 = 2; // virtio block
pub const VIRTIO_ID_CONSOLE: u16 = 3; // virtio console
pub const VIRTIO_ID_RNG: u16 = 4; // virtio rng

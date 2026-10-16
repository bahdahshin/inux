// SPDX-License-Identifier: GPL-2.0 WITH Linux-syscall-note
//! The legacy virtio-pci register layout.
//!
//! A subset of `include/uapi/linux/virtio_pci.h`: the legacy (virtio 0.9.5)
//! register block at the start of BAR0's I/O space. Modern virtio-pci (the
//! capability-based MMIO layout) is not ported; QEMU's transitional devices
//! still expose this legacy block.

/// 32-bit read-only feature bits the device offers.
pub const VIRTIO_PCI_HOST_FEATURES: u16 = 0;
/// 32-bit feature bits the driver accepts.
pub const VIRTIO_PCI_GUEST_FEATURES: u16 = 4;
/// 32-bit page frame number of the selected queue's vring.
pub const VIRTIO_PCI_QUEUE_PFN: u16 = 8;
/// 16-bit read-only size of the selected queue (0 means none).
pub const VIRTIO_PCI_QUEUE_NUM: u16 = 12;
/// 16-bit queue selector.
pub const VIRTIO_PCI_QUEUE_SEL: u16 = 14;
/// 16-bit queue-notify: write a queue index to kick the device.
pub const VIRTIO_PCI_QUEUE_NOTIFY: u16 = 16;
/// 8-bit device status (`VIRTIO_CONFIG_S_*`).
pub const VIRTIO_PCI_STATUS: u16 = 18;
/// 8-bit interrupt status, read to acknowledge (bit 0: used ring, bit 1: config).
pub const VIRTIO_PCI_ISR: u16 = 19;
/// Offset of the device-specific config block when MSI-X is off.
pub const VIRTIO_PCI_CONFIG_OFF: u16 = 20;

/// The queue address is given as a page frame number: `phys >> 12`.
pub const VIRTIO_PCI_QUEUE_ADDR_SHIFT: u32 = 12;
/// The legacy vring is aligned to a page.
pub const VIRTIO_PCI_VRING_ALIGN: usize = 4096;

/// The PCI vendor ID shared by all virtio devices (Red Hat / Qumranet).
pub const VIRTIO_PCI_VENDOR: u16 = 0x1af4;
/// Transitional virtio devices use PCI device IDs 0x1000..=0x103f.
pub const VIRTIO_PCI_LEGACY_ID_MIN: u16 = 0x1000;
pub const VIRTIO_PCI_LEGACY_ID_MAX: u16 = 0x103f;

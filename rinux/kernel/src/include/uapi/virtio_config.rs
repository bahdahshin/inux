// SPDX-License-Identifier: GPL-2.0 WITH Linux-syscall-note
//! Virtio device status bits.
//!
//! A subset of `include/uapi/linux/virtio_config.h`: the `status` field
//! values the driver walks through while bringing a device up. The feature
//! bit definitions are not ported (the RNG driver negotiates no features).

/// The driver has found the device and recognized it as valid virtio.
pub const VIRTIO_CONFIG_S_ACKNOWLEDGE: u8 = 1;
/// The driver knows how to drive the device.
pub const VIRTIO_CONFIG_S_DRIVER: u8 = 2;
/// The driver is set up and ready to drive the device.
pub const VIRTIO_CONFIG_S_DRIVER_OK: u8 = 4;
/// The driver has acknowledged all the features it understands (virtio 1.0).
pub const VIRTIO_CONFIG_S_FEATURES_OK: u8 = 8;
/// The device has experienced an error from which it can't recover.
pub const VIRTIO_CONFIG_S_NEEDS_RESET: u8 = 0x40;
/// Something went wrong and the driver has given up on the device.
pub const VIRTIO_CONFIG_S_FAILED: u8 = 0x80;

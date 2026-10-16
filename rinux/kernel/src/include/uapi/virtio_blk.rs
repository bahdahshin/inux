// SPDX-License-Identifier: GPL-2.0 WITH Linux-syscall-note
//! The virtio block device interface.
//!
//! A subset of `include/uapi/linux/virtio_blk.h`: the request header, the
//! read/write command types and the completion status, plus the config
//! offset of the device capacity. The feature bits, discard/write-zeroes,
//! zoned and SCSI-passthrough requests are not ported.

/// A block is 512 bytes; the `sector` field counts these.
pub const SECTOR_SIZE: usize = 512;

/// `struct virtio_blk_outhdr`: the header that leads every request. 16 bytes.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct VirtioBlkOutHdr {
    /// `VIRTIO_BLK_T_*`.
    pub type_: u32,
    /// I/O priority.
    pub ioprio: u32,
    /// Starting sector (512-byte units).
    pub sector: u64,
}

/// Read from the device.
pub const VIRTIO_BLK_T_IN: u32 = 0;
/// Write to the device.
pub const VIRTIO_BLK_T_OUT: u32 = 1;

/// The request completed successfully.
pub const VIRTIO_BLK_S_OK: u8 = 0;
/// The device hit an I/O error.
pub const VIRTIO_BLK_S_IOERR: u8 = 1;
/// The device does not support the request.
pub const VIRTIO_BLK_S_UNSUPP: u8 = 2;

/// Offset of the `capacity` field (in sectors, u64) in the device config.
pub const VIRTIO_BLK_CONFIG_CAPACITY: u16 = 0;

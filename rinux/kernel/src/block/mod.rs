// SPDX-License-Identifier: GPL-2.0
//! A minimal block layer, as in Linux's `block/`.
//!
//! Linux's block layer is a deep stack: `bio`s coalesced into `request`s on
//! a `request_queue`, an I/O scheduler, and `gendisk`/`block_device` on top.
//! rinux reduces this to the one operation a boot-time reader needs: read a
//! run of 512-byte sectors synchronously. A driver implements
//! [`BlockDevice`] (the role of `block_device_operations` in
//! `include/linux/blkdev.h`); higher layers read sectors through the trait.

use alloc::boxed::Box;

use rinux_lib::errno::Result;

/// A device that stores data in fixed 512-byte sectors.
pub trait BlockDevice {
    /// The device's capacity in 512-byte sectors.
    fn nr_sectors(&self) -> u64;

    /// Reads consecutive sectors starting at `sector` into `buf`, whose
    /// length must be a non-zero multiple of 512.
    fn read(&self, sector: u64, buf: &mut [u8]) -> Result;
}

/// Probes for the boot block device (`add_disk` time): the first driver that
/// binds wins. Only virtio-blk is supported so far.
pub fn probe_boot_blockdev() -> Option<Box<dyn BlockDevice>> {
    crate::drivers::block::virtio_blk::VirtioBlk::probe()
        .map(|d| Box::new(d) as Box<dyn BlockDevice>)
}

/// Probes every block device on the machine, as `add_disk` would register
/// each. Only virtio-blk is supported so far.
pub fn probe_all_blockdevs() -> alloc::vec::Vec<Box<dyn BlockDevice>> {
    crate::drivers::block::virtio_blk::VirtioBlk::probe_all()
        .into_iter()
        .map(|d| Box::new(d) as Box<dyn BlockDevice>)
        .collect()
}

pub mod partitions;

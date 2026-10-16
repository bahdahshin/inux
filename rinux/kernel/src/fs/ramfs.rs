// SPDX-License-Identifier: GPL-2.0
//! ramfs: the RAM-backed root filesystem.
//!
//! A partial port of `fs/ramfs/` and the `simple_*` helpers of `fs/libfs.c`.
//! ramfs stores a regular file's data directly in its inode (Linux uses the
//! page cache) and a directory's entries in its inode map. There is no
//! writeback, no size limit and no eviction -- it lives entirely in memory,
//! as ramfs does.
//!
//! The file operations here just forward to the ramfs read/write cores in
//! [`crate::fs::vfs`], so ramfs files go through the same `file_operations`
//! vtable as every other filesystem.

use rinux_lib::errno::Result;

use crate::fs::vfs::{init_rootfs, ramfs_read_at, ramfs_write_at};
use crate::include::linux::fs::FileOperations;

/// `ramfs_file_operations`.
pub struct RamfsFop;

impl FileOperations for RamfsFop {
    fn read(&self, ino: u64, pos: u64, buf: &mut [u8]) -> Result<usize> {
        ramfs_read_at(ino, pos, buf)
    }

    fn write(&self, ino: u64, pos: u64, data: &[u8]) -> Result<usize> {
        ramfs_write_at(ino, pos, data)
    }
}

/// The shared ramfs file-operations instance (`i_fop`).
pub static RAMFS_FOP: RamfsFop = RamfsFop;

/// `init_mount("rootfs")`: bring up the ramfs root. Mounted as `/` very
/// early, as Linux mounts rootfs before unpacking the initramfs.
pub fn mount_rootfs() {
    init_rootfs();
}

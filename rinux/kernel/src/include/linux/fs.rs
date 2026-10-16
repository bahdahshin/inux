// SPDX-License-Identifier: GPL-2.0
//! Filesystem types.
//!
//! Ported from `include/uapi/linux/stat.h`, `include/uapi/asm-generic/fcntl.h`
//! and the `file_operations`/`inode` shapes of `include/linux/fs.h`.
//!
//! rinux's VFS keeps inodes in an arena under one lock rather than the full
//! dcache and mount tree (see [`crate::fs::vfs`]); the operation vtable
//! ([`FileOperations`]) is kept, so ramfs and procfs dispatch through it as
//! in Linux.

use alloc::vec::Vec;

use rinux_lib::errno::Result;

// File type and mode bits (`stat.h`).
pub const S_IFMT: u32 = 0o170000;
pub const S_IFSOCK: u32 = 0o140000;
pub const S_IFLNK: u32 = 0o120000;
pub const S_IFREG: u32 = 0o100000;
pub const S_IFBLK: u32 = 0o060000;
pub const S_IFDIR: u32 = 0o040000;
pub const S_IFCHR: u32 = 0o020000;
pub const S_IFIFO: u32 = 0o010000;

#[inline]
pub const fn s_isdir(m: u32) -> bool {
    m & S_IFMT == S_IFDIR
}
#[inline]
pub const fn s_isreg(m: u32) -> bool {
    m & S_IFMT == S_IFREG
}
#[inline]
pub const fn s_islnk(m: u32) -> bool {
    m & S_IFMT == S_IFLNK
}

// Open flags (`fcntl.h`).
pub const O_RDONLY: u32 = 0;
pub const O_WRONLY: u32 = 1 << 0;
pub const O_RDWR: u32 = 1 << 1;
pub const O_CREAT: u32 = 1 << 6;
pub const O_TRUNC: u32 = 1 << 9;
pub const O_APPEND: u32 = 1 << 10;
pub const O_DIRECTORY: u32 = 1 << 16;

// Directory-entry types (`getdents`).
pub const DT_UNKNOWN: u8 = 0;
pub const DT_DIR: u8 = 4;
pub const DT_REG: u8 = 8;
pub const DT_LNK: u8 = 10;

// `lseek` whence (`fs.h`).
pub const SEEK_SET: u32 = 0;
pub const SEEK_CUR: u32 = 1;
pub const SEEK_END: u32 = 2;

/// `PATH_MAX`.
pub const PATH_MAX: usize = 4096;

/// A directory entry returned by a readdir (`struct dirent` essentials).
pub struct DirEntry {
    pub name: Vec<u8>,
    pub ino: u64,
    pub d_type: u8,
}

/// `struct file_operations`: how reads and writes reach a file's data.
///
/// The methods take the inode's number and the VFS so an implementation can
/// reach its backing store; ramfs copies the inode's byte buffer and procfs
/// generates content on the fly.
pub trait FileOperations: Sync {
    /// `read()`: copy up to `buf.len()` bytes of inode `ino` starting at
    /// `pos`. Returns the number copied (0 at end of file).
    fn read(&self, ino: u64, pos: u64, buf: &mut [u8]) -> Result<usize>;

    /// `write()`: write `data` to inode `ino` at `pos`, growing it. The
    /// default rejects writes (read-only files such as procfs).
    fn write(&self, _ino: u64, _pos: u64, _data: &[u8]) -> Result<usize> {
        Err(rinux_lib::errno::EINVAL)
    }
}

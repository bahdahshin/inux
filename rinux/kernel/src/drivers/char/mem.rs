// SPDX-License-Identifier: GPL-2.0
//! The null, zero and full memory character devices.
//!
//! A small port of the `/dev/null`, `/dev/zero` and `/dev/full` devices of
//! `drivers/char/mem.c`: a bit bucket, an endless source of zero bytes, and a
//! device that is always full. Linux registers these as char devices and lets
//! devtmpfs create the nodes; rinux grafts the nodes straight into the rootfs
//! and gives each inode the matching `file_operations`.
//!
//! Not ported: `/dev/mem`, `/dev/port`, `/dev/kmem` and the mmap paths.

use alloc::vec::Vec;

use rinux_lib::errno::{ENOSPC, Result};

use crate::fs::vfs::{InodeKind, graft, vfs_mkdir};
use crate::include::linux::fs::{FileOperations, S_IFCHR};

/// `/dev/null`: reads return end-of-file, writes are discarded.
pub struct NullFop;
impl FileOperations for NullFop {
    fn read(&self, _ino: u64, _pos: u64, _buf: &mut [u8]) -> Result<usize> {
        Ok(0) // read_null()
    }
    fn write(&self, _ino: u64, _pos: u64, data: &[u8]) -> Result<usize> {
        Ok(data.len()) // write_null()
    }
}

/// `/dev/zero`: reads yield zero bytes, writes are discarded.
pub struct ZeroFop;
impl FileOperations for ZeroFop {
    fn read(&self, _ino: u64, _pos: u64, buf: &mut [u8]) -> Result<usize> {
        buf.fill(0); // read_zero()
        Ok(buf.len())
    }
    fn write(&self, _ino: u64, _pos: u64, data: &[u8]) -> Result<usize> {
        Ok(data.len()) // write_zero()
    }
}

/// `/dev/full`: reads yield zero bytes, writes always fail with `ENOSPC`.
pub struct FullFop;
impl FileOperations for FullFop {
    fn read(&self, _ino: u64, _pos: u64, buf: &mut [u8]) -> Result<usize> {
        buf.fill(0);
        Ok(buf.len())
    }
    fn write(&self, _ino: u64, _pos: u64, _data: &[u8]) -> Result<usize> {
        Err(ENOSPC) // write_full()
    }
}

static NULL_FOP: NullFop = NullFop;
static ZERO_FOP: ZeroFop = ZeroFop;
static FULL_FOP: FullFop = FullFop;

/// `chr_dev_init()`: create `/dev` and register the memory devices. A char
/// device has no stored data, so each node is an empty `Reg` inode whose
/// `file_operations` provide the behaviour.
pub fn mem_init() {
    let _ = vfs_mkdir(b"/dev", 0o755);
    let _ = graft(
        b"/dev",
        b"null",
        S_IFCHR | 0o666,
        &NULL_FOP,
        InodeKind::Reg(Vec::new()),
    );
    let _ = graft(
        b"/dev",
        b"zero",
        S_IFCHR | 0o666,
        &ZERO_FOP,
        InodeKind::Reg(Vec::new()),
    );
    let _ = graft(
        b"/dev",
        b"full",
        S_IFCHR | 0o666,
        &FULL_FOP,
        InodeKind::Reg(Vec::new()),
    );
}

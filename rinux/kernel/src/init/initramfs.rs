// SPDX-License-Identifier: GPL-2.0
//! Unpacking the initramfs.
//!
//! A partial port of `init/initramfs.c`: parse a `newc`-format cpio archive
//! and populate the rootfs with its directories, regular files and
//! symlinks. The whole archive is in memory (a boot-loader module), so the
//! streaming state machine of the C code is unrolled into a direct loop
//! over the entries; the header parsing and 4-byte alignment follow the C
//! exactly.
//!
//! Not ported: gzip/xz/zstd decompression (the archive must be plain cpio),
//! hard-link coalescing (`maybe_link`), device nodes, and the fallback
//! `CONFIG_INITRAMFS_SOURCE` built-in archive.

use crate::fs::vfs::{filp_open, vfs_mkdir, vfs_symlink, vfs_write};
use crate::include::linux::fs::*;

/// Length of a `newc` header: "070701" plus 13 eight-digit hex fields.
const CPIO_HDRLEN: usize = 6 + 13 * 8;

/// `N_ALIGN`: bytes a name of `name_len` occupies after the header, padded
/// so the data that follows the 110-byte header is 4-byte aligned.
fn n_align(name_len: usize) -> usize {
    (((name_len) + 1) & !3) + 2
}

fn align4(n: usize) -> usize {
    (n + 3) & !3
}

/// Parses one 8-digit hex field (`parse_header` via `hex2bin`/`be32`).
fn hex_field(s: &[u8]) -> Option<u32> {
    if s.len() < 8 {
        return None;
    }
    let mut v: u32 = 0;
    for &c in &s[..8] {
        v = v << 4 | (c as char).to_digit(16)?;
    }
    Some(v)
}

/// `unpack_to_rootfs()`: populate the rootfs from the cpio archive `buf`.
///
/// Returns the number of entries unpacked, or an error message.
pub fn unpack_to_rootfs(buf: &[u8]) -> Result<usize, &'static str> {
    let mut pos = 0;
    let mut count = 0;

    while pos + CPIO_HDRLEN <= buf.len() {
        let hdr = &buf[pos..pos + CPIO_HDRLEN];
        if &hdr[..6] != b"070701" {
            if &hdr[..6] == b"070702" {
                // New ASCII with checksum: fields are laid out the same.
            } else {
                return Err("no cpio magic");
            }
        }

        // Fields after the 6-byte magic: ino, mode, uid, gid, nlink, mtime,
        // filesize, devmajor, devminor, rdevmajor, rdevminor, namesize, check.
        let field = |i: usize| hex_field(&hdr[6 + i * 8..]).ok_or("damaged header");
        let mode = field(1)?;
        let body_len = field(6)? as usize;
        let name_len = field(11)? as usize;

        let name_off = pos + CPIO_HDRLEN;
        if name_off + name_len > buf.len() || name_len == 0 {
            return Err("truncated archive");
        }
        let name = &buf[name_off..name_off + name_len - 1]; // drop trailing NUL
        let data_off = pos + CPIO_HDRLEN + n_align(name_len);
        let next = align4(data_off + body_len);

        if name == b"TRAILER!!!" {
            break;
        }

        // Make the name absolute under the root.
        let mut path = alloc::vec::Vec::with_capacity(name.len() + 1);
        path.push(b'/');
        path.extend_from_slice(name);

        if s_isdir(mode) {
            let _ = vfs_mkdir(&path, mode & 0o777);
        } else if s_isreg(mode) {
            if data_off + body_len > buf.len() {
                return Err("truncated file data");
            }
            let data = &buf[data_off..data_off + body_len];
            let mut file = filp_open(&path, O_WRONLY | O_CREAT | O_TRUNC, mode & 0o777)
                .map_err(|_| "cannot create file")?;
            if vfs_write(&mut file, data).map_err(|_| "write error")? != body_len {
                return Err("short write");
            }
        } else if s_islnk(mode) {
            if data_off + body_len > buf.len() {
                return Err("truncated symlink");
            }
            let target = &buf[data_off..data_off + body_len];
            let _ = vfs_symlink(&path, target);
        }
        // Other types (devices, fifos) are skipped.

        count += 1;
        pos = next;
    }
    Ok(count)
}

/// `populate_rootfs()`: unpack the boot loader's initramfs module, if any.
///
/// Returns the number of entries, or `None` if no initramfs was provided.
pub fn populate_rootfs() -> Option<usize> {
    let module = crate::arch::x86::kernel::setup::boot_info()?
        .modules()
        .next()?;
    // SAFETY: GRUB loaded the module into identity-mapped low memory and it
    // stays there; the range is within the module's declared bounds.
    let buf = unsafe {
        core::slice::from_raw_parts(
            module.start as *const u8,
            (module.end - module.start) as usize,
        )
    };
    match unpack_to_rootfs(buf) {
        Ok(n) => Some(n),
        Err(msg) => {
            pr_err!("initramfs: {}", msg);
            Some(0)
        }
    }
}

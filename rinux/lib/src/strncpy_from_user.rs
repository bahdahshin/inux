// SPDX-License-Identifier: GPL-2.0
//! Copy a NUL-terminated string from user memory, bounded by a count.
//!
//! Ported from `lib/strncpy_from_user.c`. The C code reads user memory a word
//! at a time when it can, masking the bytes after the NUL, and falls back to
//! bytes near a fault. Those are speed and the BPF map-key rule; the result is
//! the same as the byte loop, which is what is written here. User memory is
//! reached through `read`, which returns `None` where C takes a page fault or
//! hits the end of the address space; the page-table walk is the kernel's job.

use crate::errno::{EFAULT, Errno};

/// `strncpy_from_user()`: copy from user memory into `dst`, stopping at a NUL
/// or after `count` bytes. Returns the length of the string, not counting the
/// NUL. If `count` bytes arrive with no NUL, returns `count`.
///
/// `read(addr)` gives the user byte at `addr`, or `None` for a fault.
/// A fault before `count` bytes is `EFAULT`, as in C.
pub fn strncpy_from_user(
    dst: &mut [u8],
    count: usize,
    mut read: impl FnMut(usize) -> Option<u8>,
) -> Result<usize, Errno> {
    if count == 0 {
        return Ok(0);
    }
    let max = count.min(dst.len());
    for res in 0..max {
        let Some(c) = read(res) else {
            return Err(EFAULT);
        };
        dst[res] = c;
        if c == 0 {
            return Ok(res);
        }
    }
    // Hit `count` with no NUL: that is the caller's full request.
    Ok(max)
}

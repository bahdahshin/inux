// SPDX-License-Identifier: GPL-2.0-only
//! The allocating helpers that the rest of `lib/` leaves out.
//!
//! Ported from `lib/vsprintf.c` (`kasprintf()`), `lib/parser.c`
//! (`match_strdup()`) and `lib/bitmap.c` (`bitmap_alloc()` and its family).
//!
//! This module is behind the `alloc` feature. The kernel enables it and
//! routes the `alloc` crate through its `kmalloc`-backed global allocator,
//! so these return ordinary `Vec`s instead of `kmalloc`ed pointers.
//!
//! Deviations from C:
//! - `kasprintf()` returns the formatted bytes without the trailing NUL. The
//!   length is the same, and the caller owns the buffer.
//! - `bitmap_alloc()` returns zeroed words. C leaves them uninitialised, and
//!   zeroing is the defined value that callers rely on.
//! - `bitmap_free()` is `drop`, so there is no separate function.

use alloc::vec::Vec;

use crate::bitmap_str::{bitmap_parse, bitmap_parselist};
use crate::bitops::bits_to_longs;
use crate::errno::Result;
use crate::uaccess::{UserMemory, copy_in};
use crate::vsprintf::{Arg, vsnprintf};

/// `kasprintf()`: format into a freshly allocated buffer, sized by a first
/// pass that counts the bytes.
pub fn kasprintf(fmt: impl AsRef<[u8]>, args: &[Arg<'_>]) -> Vec<u8> {
    let fmt = fmt.as_ref();
    let len = vsnprintf(&mut [], fmt, args);
    // One extra byte for the NUL that vsnprintf always writes, then drop it.
    let mut buf = alloc::vec![0u8; len + 1];
    vsnprintf(&mut buf, fmt, args);
    buf.truncate(len);
    buf
}

/// `match_strdup()`: copy a matched token. The C substring is a pointer
/// range; here it is a slice, so the copy is the slice's bytes.
pub fn match_strdup(s: &[u8]) -> Vec<u8> {
    s.to_vec()
}

/// `bitmap_alloc()`: a bitmap of `nbits` bits, rounded up to whole words.
/// The words are zeroed; see the module docs.
pub fn bitmap_alloc(nbits: usize) -> Vec<u64> {
    alloc::vec![0u64; bits_to_longs(nbits)]
}

/// `bitmap_zalloc()`: the same as [`bitmap_alloc`], which already zeroes.
pub fn bitmap_zalloc(nbits: usize) -> Vec<u64> {
    bitmap_alloc(nbits)
}

/// `memdup_user_nul()`: copies `len` bytes from user memory into a buffer
/// with a NUL after them. A copy that faults is `EFAULT`.
pub fn memdup_user_nul(user: &impl UserMemory, src: usize, len: usize) -> Result<Vec<u8>> {
    let mut buf = alloc::vec![0u8; len + 1];
    copy_in(user, &mut buf[..len], src)?;
    Ok(buf)
}

/// `bitmap_parse_user()`: copies `ulen` bytes of user memory and parses them
/// as [`bitmap_parse`] does. The string is NUL-terminated by the copy.
pub fn bitmap_parse_user(
    user: &impl UserMemory,
    src: usize,
    ulen: usize,
    mask: &mut [u64],
    nmaskbits: usize,
) -> Result<()> {
    let buf = memdup_user_nul(user, src, ulen)?;
    bitmap_parse(&buf, usize::MAX, mask, nmaskbits)
}

/// `bitmap_parselist_user()`: as [`bitmap_parse_user`], for the list format
/// that [`bitmap_parselist`] reads.
pub fn bitmap_parselist_user(
    user: &impl UserMemory,
    src: usize,
    ulen: usize,
    mask: &mut [u64],
    nmaskbits: usize,
) -> Result<()> {
    let buf = memdup_user_nul(user, src, ulen)?;
    bitmap_parselist(&buf, mask, nmaskbits)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kasprintf_sizes_the_buffer_to_the_output() {
        let s = kasprintf("%d-%s", &[Arg::Num(42), Arg::Str(b"x")]);
        assert_eq!(s, b"42-x");
        assert_eq!(kasprintf("", &[]), b"");
    }

    #[test]
    fn match_strdup_copies_the_token() {
        let src = b"token";
        let copy = match_strdup(src);
        assert_eq!(copy, b"token");
        assert_ne!(copy.as_ptr(), src.as_ptr());
    }

    #[test]
    fn bitmap_alloc_rounds_to_words_and_zeroes() {
        assert_eq!(bitmap_alloc(0).len(), 0);
        assert_eq!(bitmap_alloc(1).len(), 1);
        assert_eq!(bitmap_alloc(64).len(), 1);
        assert_eq!(bitmap_alloc(65).len(), 2);
        assert!(bitmap_zalloc(130).iter().all(|&w| w == 0));
    }
}

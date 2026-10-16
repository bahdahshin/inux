// SPDX-License-Identifier: GPL-2.0
//! Byte-range operations on memory-mapped I/O: fill, copy in, copy out.
//!
//! Ported from `lib/iomem_copy.c`: `memset_io()`, `memcpy_fromio()` and
//! `memcpy_toio()`. C moves a head of single bytes until the I/O address is
//! word-aligned, then whole words, then the tail. The result is the same bytes
//! in the same order as a plain byte loop, which is what is written here; the
//! word width is only a bus-access detail, so the kernel's word accessors are
//! left to it.

/// `memset_io()`: write `val` to `count` device bytes starting at offset 0,
/// through `write_byte(offset, byte)`.
pub fn memset_io(count: usize, val: u8, mut write_byte: impl FnMut(usize, u8)) {
    for off in 0..count {
        write_byte(off, val);
    }
}

/// `memcpy_fromio()`: read `dst.len()` device bytes into `dst`, through
/// `read_byte(offset)`.
pub fn memcpy_fromio(dst: &mut [u8], mut read_byte: impl FnMut(usize) -> u8) {
    for (off, slot) in dst.iter_mut().enumerate() {
        *slot = read_byte(off);
    }
}

/// `memcpy_toio()`: write `src` to device bytes, through
/// `write_byte(offset, byte)`.
pub fn memcpy_toio(src: &[u8], mut write_byte: impl FnMut(usize, u8)) {
    for (off, &b) in src.iter().enumerate() {
        write_byte(off, b);
    }
}

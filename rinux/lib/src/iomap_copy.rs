// SPDX-License-Identifier: GPL-2.0
//! Copy blocks of words to and from memory-mapped I/O.
//!
//! Ported from `lib/iomap_copy.c`: `__iowrite32_copy()`, `__ioread32_copy()` and
//! `__iowrite64_copy()`. Each copies `count` words one at a time with a raw
//! register access, so the block is not combined into wider bus cycles. The
//! kernel's `__raw_writel()` and friends are supplied as closures, because the
//! mapping is the kernel's.

/// `__iowrite32_copy()`: write `src` as 32-bit words at successive device
/// addresses, using `write(index, value)`.
pub fn iowrite32_copy(src: &[u32], mut write: impl FnMut(usize, u32)) {
    for (i, &w) in src.iter().enumerate() {
        write(i, w);
    }
}

/// `__ioread32_copy()`: read `dst.len()` 32-bit words from successive device
/// addresses, using `read(index)`.
pub fn ioread32_copy(dst: &mut [u32], mut read: impl FnMut(usize) -> u32) {
    for (i, slot) in dst.iter_mut().enumerate() {
        *slot = read(i);
    }
}

/// `__iowrite64_copy()` on a 64-bit kernel: write `src` as 64-bit words.
pub fn iowrite64_copy(src: &[u64], mut write: impl FnMut(usize, u64)) {
    for (i, &w) in src.iter().enumerate() {
        write(i, w);
    }
}

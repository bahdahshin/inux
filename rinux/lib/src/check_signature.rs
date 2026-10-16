// SPDX-License-Identifier: GPL-2.0
//! Compare a BIOS-style signature with memory that is read byte by byte.
//!
//! Ported from `lib/check_signature.c`. The C function reads an `ioremap`ed
//! MMIO address with `readb()`. The MMIO access is the caller's job here:
//! `read` returns the byte at an offset, so the same comparison works over a
//! device mapping, a test buffer or the physical-memory window.

/// `check_signature()`: true when every byte of `signature` matches the
/// bytes that `read` returns for offsets `0..signature.len()`.
pub fn check_signature(mut read: impl FnMut(usize) -> u8, signature: &[u8]) -> bool {
    signature
        .iter()
        .enumerate()
        .all(|(offset, &expected)| read(offset) == expected)
}

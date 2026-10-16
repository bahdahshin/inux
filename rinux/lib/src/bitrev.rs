// SPDX-License-Identifier: GPL-2.0-only
//! Bit-order reversal.
//!
//! Ported from `lib/bitrev.c`, `include/linux/bitrev.h` and
//! `include/asm-generic/bitops/__bitrev.h`.
//!
//! Author: Akinobu Mita <akinobu.mita@gmail.com>

include!("bitrev_table.rs");

/// `bitrev8()`: reverses the bits of a byte.
#[inline]
pub const fn bitrev8(byte: u8) -> u8 {
    BYTE_REV_TABLE[byte as usize]
}

/// `bitrev16()`: reverses the bits of a 16-bit value.
#[inline]
pub const fn bitrev16(x: u16) -> u16 {
    ((bitrev8(x as u8) as u16) << 8) | bitrev8((x >> 8) as u8) as u16
}

/// `bitrev32()`: reverses the bits of a 32-bit value.
#[inline]
pub const fn bitrev32(x: u32) -> u32 {
    ((bitrev16(x as u16) as u32) << 16) | bitrev16((x >> 16) as u16) as u32
}

/// `bitrev8x4()`: reverses the bits within each byte of a 32-bit value.
#[inline]
pub const fn bitrev8x4(x: u32) -> u32 {
    bitrev32(x.swap_bytes())
}

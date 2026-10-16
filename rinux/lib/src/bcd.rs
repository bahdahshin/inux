// SPDX-License-Identifier: GPL-2.0
//! Binary-coded decimal.
//!
//! Ported from `lib/bcd.c` and `include/linux/bcd.h`.

/// `bcd2bin()`: packed BCD byte to binary.
#[inline]
pub const fn bcd2bin(val: u8) -> u32 {
    (val & 0x0f) as u32 + (val >> 4) as u32 * 10
}

/// `bin2bcd()`: binary (0..=99) to packed BCD.
///
/// Divides by 10 with a multiply and shift, exact for the valid range.
#[inline]
pub const fn bin2bcd(val: u32) -> u8 {
    let t = (val * 103) >> 10;
    ((t << 4) | (val - t * 10)) as u8
}

/// `bcd_is_valid()`: both nibbles are decimal digits.
#[inline]
pub const fn bcd_is_valid(x: u8) -> bool {
    (x & 0x0f) < 10 && (x >> 4) < 10
}

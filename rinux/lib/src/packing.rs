// SPDX-License-Identifier: BSD-3-Clause OR GPL-2.0
//! Generic bitfield packing and unpacking.
//!
//! Ported from `lib/packing.c` and `include/linux/packing.h`, Copyright
//! 2016-2018 NXP and 2024 Vladimir Oltean. Bit numbers are logical: bit 0 is
//! the least significant bit of the packed value, and `startbit` is the more
//! significant end of the field. The packed buffer is a byte slice whose
//! length is `pbuflen`.
//!
//! The C `pack_fields_*()` and `unpack_fields_*()` helpers copy fields in
//! and out of a C struct by byte offset. That needs raw struct pointers, so
//! they are not ported; use [`pack`], [`unpack`] or [`packing`] per field.

use crate::bitrev::bitrev8;
use crate::errno::{EINVAL, ERANGE, Errno};

/// Pack and unpack with the first 32-bit word at the lowest address.
pub const QUIRK_LSW32_IS_FIRST: u8 = 1 << 2;
/// Each 32-bit word is stored little-endian.
pub const QUIRK_LITTLE_ENDIAN: u8 = 1 << 1;
/// Bits within each byte are stored MSB on the right (bit-reversed).
pub const QUIRK_MSB_ON_THE_RIGHT: u8 = 1 << 0;

/// `enum packing_op`: the direction of [`packing`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PackingOp {
    Pack,
    Unpack,
}

/// `GENMASK_ULL(h, l)`: bits `l` through `h` set, inclusive.
const fn genmask64(h: usize, l: usize) -> u64 {
    (u64::MAX >> (63 - h)) & (u64::MAX << l)
}

/// `GENMASK(h, l)` for a byte.
const fn genmask8(h: usize, l: usize) -> u8 {
    (u8::MAX >> (7 - h)) & (u8::MAX << l)
}

/// `calculate_box_addr()`: the byte offset of logical byte `box` in the
/// packed buffer of `len` bytes, honouring the word-order quirks.
fn calculate_box_addr(box_: usize, len: usize, quirks: u8) -> usize {
    let this_group = box_ / 4;
    let offset_of_group = if quirks & QUIRK_LSW32_IS_FIRST != 0 {
        this_group * 4
    } else {
        len - (this_group + 1) * 4
    };
    let group_size = core::cmp::min(4, len - offset_of_group);
    let offset_in_group = if quirks & QUIRK_LITTLE_ENDIAN != 0 {
        box_ - this_group * 4
    } else {
        group_size - (box_ - this_group * 4) - 1
    };
    offset_of_group + offset_in_group
}

/// Walks the logical bytes spanned by `startbit..=endbit`, yielding the
/// byte address and the bit windows used by the C loops.
///
/// Each item is `(box_addr, box_mask, box_end_bit, proj_mask, proj_end_bit)`.
fn boxes(
    startbit: usize,
    endbit: usize,
    pbuflen: usize,
    quirks: u8,
) -> impl Iterator<Item = (usize, u8, usize, u64, usize)> {
    let first = startbit / 8;
    let last = endbit / 8;
    (last..=first).rev().map(move |box_| {
        let box_start_bit = if box_ == first { startbit % 8 } else { 7 };
        let box_end_bit = if box_ == last { endbit % 8 } else { 0 };
        let proj_start_bit = (box_ * 8 + box_start_bit) - endbit;
        let proj_end_bit = (box_ * 8 + box_end_bit) - endbit;
        let proj_mask = genmask64(proj_start_bit, proj_end_bit);
        let box_mask = genmask8(box_start_bit, box_end_bit);
        let box_addr = calculate_box_addr(box_, pbuflen, quirks);
        (box_addr, box_mask, box_end_bit, proj_mask, proj_end_bit)
    })
}

/// `__pack()`: store the low `startbit - endbit + 1` bits of `uval`.
///
/// C warns and truncates when `uval` is too wide; this port truncates the
/// same way without the warning.
fn pack_bits(pbuf: &mut [u8], uval: u64, startbit: usize, endbit: usize, quirks: u8) {
    let pbuflen = pbuf.len();
    for (box_addr, mut box_mask, box_end_bit, proj_mask, proj_end_bit) in
        boxes(startbit, endbit, pbuflen, quirks)
    {
        let mut pval = (uval & proj_mask) >> proj_end_bit;
        pval <<= box_end_bit;
        let mut pval = pval as u8;
        if quirks & QUIRK_MSB_ON_THE_RIGHT != 0 {
            pval = bitrev8(pval);
            box_mask = box_mask.reverse_bits();
        }
        pbuf[box_addr] &= !box_mask;
        pbuf[box_addr] |= pval;
    }
}

/// `__unpack()`: read the field into the low bits of `uval`.
fn unpack_bits(pbuf: &[u8], uval: &mut u64, startbit: usize, endbit: usize, quirks: u8) {
    let pbuflen = pbuf.len();
    *uval = 0;
    for (box_addr, box_mask, box_end_bit, proj_mask, proj_end_bit) in
        boxes(startbit, endbit, pbuflen, quirks)
    {
        let mut pval = pbuf[box_addr];
        if quirks & QUIRK_MSB_ON_THE_RIGHT != 0 {
            pval = bitrev8(pval);
        }
        let pval = u64::from(pval & box_mask) >> box_end_bit;
        let pval = pval << proj_end_bit;
        *uval &= !proj_mask;
        *uval |= pval;
    }
}

/// `pack()`: store the field `startbit..=endbit` of `uval` into `pbuf`.
pub fn pack(
    pbuf: &mut [u8],
    uval: u64,
    startbit: usize,
    endbit: usize,
    quirks: u8,
) -> Result<(), Errno> {
    // startbit is the more significant end, and must lie inside the buffer.
    if startbit < endbit || startbit >= 8 * pbuf.len() {
        return Err(EINVAL);
    }
    if startbit - endbit >= 64 {
        return Err(ERANGE);
    }
    pack_bits(pbuf, uval, startbit, endbit, quirks);
    Ok(())
}

/// `unpack()`: read the field `startbit..=endbit` of `pbuf` into `uval`.
pub fn unpack(
    pbuf: &[u8],
    uval: &mut u64,
    startbit: usize,
    endbit: usize,
    quirks: u8,
) -> Result<(), Errno> {
    if startbit < endbit || startbit >= 8 * pbuf.len() {
        return Err(EINVAL);
    }
    if startbit - endbit + 1 > 64 {
        return Err(ERANGE);
    }
    unpack_bits(pbuf, uval, startbit, endbit, quirks);
    Ok(())
}

/// `packing()`: [`pack`] or [`unpack`] depending on `op`.
///
/// C takes `u64 *uval` for both directions; here `Pack` reads `*uval` and
/// `Unpack` writes it, as the C code does.
pub fn packing(
    pbuf: &mut [u8],
    uval: &mut u64,
    startbit: usize,
    endbit: usize,
    op: PackingOp,
    quirks: u8,
) -> Result<(), Errno> {
    match op {
        PackingOp::Pack => pack(pbuf, *uval, startbit, endbit, quirks),
        PackingOp::Unpack => unpack(pbuf, uval, startbit, endbit, quirks),
    }
}

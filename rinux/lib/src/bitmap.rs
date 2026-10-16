// SPDX-License-Identifier: GPL-2.0-only
//! Bitmaps: arrays of bits in `u64` words.
//!
//! Ported from `lib/bitmap.c`, `lib/bitmap-str.c`, `include/linux/bitmap.h`
//! and `include/linux/bitmap-str.h`.
//!
//! A bitmap of `nbits` bits is a `[u64]` slice with at least
//! [`bits_to_longs`]`(nbits)` words; see [`crate::bitops`]. As in C, the
//! unused bits of the last word are "don't care": operations may change
//! them, and results never depend on them.
//!
//! The C header picks faster code when `nbits` is a compile-time constant
//! of at most one word; those paths give the same results and are not
//! ported. The `__bitmap_*()` out-of-line helpers are the functions
//! themselves.
//!
//! Differences from C:
//!
//! - `bitmap_alloc()` and friends need the allocator; a bitmap here is any
//!   `[u64]` the caller owns.
//! - In-place calls of [`bitmap_remap`], [`bitmap_onto`] and [`bitmap_fold`]
//!   (where C returns early when `dst == src`) cannot be expressed: Rust
//!   does not let the destination alias a source.
//! - [`bitmap_cut`] takes separate source and destination; the overlapping
//!   case is [`bitmap_cut_in_place`] after a `copy_within()`.
//! - `bitmap_parse_user()` and `bitmap_parselist_user()` wait for uaccess.
//! - [`bitmap_print_bitmask_to_buf`] and [`bitmap_print_list_to_buf`] print
//!   straight into the window of the output that is asked for instead of
//!   formatting the whole string with `kasprintf()` first.

use crate::bitops::{
    BITS_PER_LONG, assign_bit, bit_word, bits_to_longs, bits_to_u32, set_bit, test_bit,
};
use crate::ctype::isspace;
use crate::errno::{EBUSY, EINVAL, ENOMEM, EOVERFLOW, ERANGE, Result};
use crate::find_bit::{
    find_first_bit, find_first_zero_bit, find_last_bit, find_next_bit, find_next_zero_bit,
    find_nth_bit, for_each_set_bit,
};
use crate::hexdump::hex_to_bin;
use crate::kstrtox::{KSTRTOX_OVERFLOW, parse_integer};
use crate::string::{strncasecmp, strnchrnul};

#[inline]
fn at(s: &[u8], i: usize) -> u8 {
    s.get(i).copied().unwrap_or(0)
}

/// `BITMAP_FIRST_WORD_MASK(start)`: the bits of the first word from `start` on.
#[inline]
pub const fn bitmap_first_word_mask(start: usize) -> u64 {
    !0u64 << (start & (BITS_PER_LONG - 1))
}

/// `BITMAP_LAST_WORD_MASK(nbits)`: the bits of the last word below `nbits`.
#[inline]
pub const fn bitmap_last_word_mask(nbits: usize) -> u64 {
    !0u64 >> (nbits.wrapping_neg() & (BITS_PER_LONG - 1))
}

/// `bitmap_size(nbits)`: bytes in the words of an `nbits` bitmap.
#[inline]
pub const fn bitmap_size(nbits: usize) -> usize {
    bits_to_longs(nbits) * 8
}

/// `bitmap_zero()`: clears every word that holds one of the `nbits` bits.
pub fn bitmap_zero(dst: &mut [u64], nbits: usize) {
    dst[..bits_to_longs(nbits)].fill(0);
}

/// `bitmap_fill()`: sets every word that holds one of the `nbits` bits.
pub fn bitmap_fill(dst: &mut [u64], nbits: usize) {
    dst[..bits_to_longs(nbits)].fill(!0);
}

/// `bitmap_copy()`: copies the words that hold the `nbits` bits.
pub fn bitmap_copy(dst: &mut [u64], src: &[u64], nbits: usize) {
    let len = bits_to_longs(nbits);
    dst[..len].copy_from_slice(&src[..len]);
}

/// `bitmap_copy_clear_tail()`: [`bitmap_copy`], then clears the bits of the
/// last word beyond `nbits`.
pub fn bitmap_copy_clear_tail(dst: &mut [u64], src: &[u64], nbits: usize) {
    bitmap_copy(dst, src, nbits);
    if nbits % BITS_PER_LONG != 0 {
        dst[nbits / BITS_PER_LONG] &= bitmap_last_word_mask(nbits);
    }
}

/// `bitmap_copy_and_extend()`: copies `count` bits and clears the rest of a
/// `size`-bit destination.
pub fn bitmap_copy_and_extend(to: &mut [u64], from: &[u64], count: usize, size: usize) {
    let copy = bits_to_longs(count);
    to[..copy].copy_from_slice(&from[..copy]);
    if count % BITS_PER_LONG != 0 {
        to[copy - 1] &= bitmap_last_word_mask(count);
    }
    to[copy..bits_to_longs(size)].fill(0);
}

/// `bitmap_from_arr32()`: builds a bitmap from 32-bit words.
pub fn bitmap_from_arr32(bitmap: &mut [u64], buf: &[u32], nbits: usize) {
    let halfwords = nbits.div_ceil(32);
    let mut i = 0;
    while i < halfwords {
        bitmap[i / 2] = buf[i] as u64;
        i += 1;
        if i < halfwords {
            bitmap[i / 2] |= (buf[i] as u64) << 32;
        }
        i += 1;
    }
    // Clear tail bits in last word beyond nbits.
    if nbits % BITS_PER_LONG != 0 {
        bitmap[(halfwords - 1) / 2] &= bitmap_last_word_mask(nbits);
    }
}

/// `bitmap_to_arr32()`: splits a bitmap into 32-bit words.
pub fn bitmap_to_arr32(buf: &mut [u32], bitmap: &[u64], nbits: usize) {
    let halfwords = nbits.div_ceil(32);
    let mut i = 0;
    while i < halfwords {
        buf[i] = bitmap[i / 2] as u32;
        i += 1;
        if i < halfwords {
            buf[i] = (bitmap[i / 2] >> 32) as u32;
        }
        i += 1;
    }
    // Clear tail bits in last element of array beyond nbits.
    if nbits % BITS_PER_LONG != 0 {
        buf[halfwords - 1] &= u32::MAX >> (nbits.wrapping_neg() & 31);
    }
}

/// `bitmap_from_arr64()`: a bitmap already is an array of `u64`.
pub fn bitmap_from_arr64(bitmap: &mut [u64], buf: &[u64], nbits: usize) {
    bitmap_copy_clear_tail(bitmap, buf, nbits);
}

/// `bitmap_to_arr64()`: a bitmap already is an array of `u64`.
pub fn bitmap_to_arr64(buf: &mut [u64], bitmap: &[u64], nbits: usize) {
    bitmap_copy_clear_tail(buf, bitmap, nbits);
}

/// `bitmap_from_u64()`: a 64-bit value as a bitmap.
pub fn bitmap_from_u64(dst: &mut [u64], mask: u64) {
    bitmap_from_arr64(dst, &[mask], 64);
}

/// `bitmap_and()`: `dst = src1 & src2`. Returns true if any bit is set.
pub fn bitmap_and(dst: &mut [u64], src1: &[u64], src2: &[u64], nbits: usize) -> bool {
    let lim = nbits / BITS_PER_LONG;
    let mut result = 0;
    for k in 0..lim {
        dst[k] = src1[k] & src2[k];
        result |= dst[k];
    }
    if nbits % BITS_PER_LONG != 0 {
        dst[lim] = src1[lim] & src2[lim] & bitmap_last_word_mask(nbits);
        result |= dst[lim];
    }
    result != 0
}

/// `bitmap_or()`: `dst = src1 | src2`.
pub fn bitmap_or(dst: &mut [u64], src1: &[u64], src2: &[u64], nbits: usize) {
    for k in 0..bits_to_longs(nbits) {
        dst[k] = src1[k] | src2[k];
    }
}

/// `bitmap_xor()`: `dst = src1 ^ src2`.
pub fn bitmap_xor(dst: &mut [u64], src1: &[u64], src2: &[u64], nbits: usize) {
    for k in 0..bits_to_longs(nbits) {
        dst[k] = src1[k] ^ src2[k];
    }
}

/// `bitmap_andnot()`: `dst = src1 & !src2`. Returns true if any bit is set.
pub fn bitmap_andnot(dst: &mut [u64], src1: &[u64], src2: &[u64], nbits: usize) -> bool {
    let lim = nbits / BITS_PER_LONG;
    let mut result = 0;
    for k in 0..lim {
        dst[k] = src1[k] & !src2[k];
        result |= dst[k];
    }
    if nbits % BITS_PER_LONG != 0 {
        dst[lim] = src1[lim] & !src2[lim] & bitmap_last_word_mask(nbits);
        result |= dst[lim];
    }
    result != 0
}

/// `bitmap_complement()`: `dst = !src`.
pub fn bitmap_complement(dst: &mut [u64], src: &[u64], nbits: usize) {
    for k in 0..bits_to_longs(nbits) {
        dst[k] = !src[k];
    }
}

/// `BITMAP_WEIGHT()`: sums the weights of the words `fetch` produces.
#[inline]
fn bitmap_weight_by(mut fetch: impl FnMut(usize) -> u64, nbits: usize) -> usize {
    let mut w = 0;
    let lim = nbits / BITS_PER_LONG;
    for idx in 0..lim {
        w += fetch(idx).count_ones() as usize;
    }
    if nbits % BITS_PER_LONG != 0 {
        w += (fetch(lim) & bitmap_last_word_mask(nbits)).count_ones() as usize;
    }
    w
}

/// `bitmap_weighted_or()`: `dst = src1 | src2`; returns the weight of `dst`.
pub fn bitmap_weighted_or(dst: &mut [u64], src1: &[u64], src2: &[u64], nbits: usize) -> usize {
    bitmap_weight_by(
        |i| {
            dst[i] = src1[i] | src2[i];
            dst[i]
        },
        nbits,
    )
}

/// `bitmap_weighted_xor()`: `dst = src1 ^ src2`; returns the weight of `dst`.
pub fn bitmap_weighted_xor(dst: &mut [u64], src1: &[u64], src2: &[u64], nbits: usize) -> usize {
    bitmap_weight_by(
        |i| {
            dst[i] = src1[i] ^ src2[i];
            dst[i]
        },
        nbits,
    )
}

/// `__bitmap_equal()`: are the first `nbits` bits equal?
pub fn __bitmap_equal(bitmap1: &[u64], bitmap2: &[u64], bits: usize) -> bool {
    let lim = bits / BITS_PER_LONG;
    if bitmap1[..lim] != bitmap2[..lim] {
        return false;
    }
    if bits % BITS_PER_LONG != 0 && (bitmap1[lim] ^ bitmap2[lim]) & bitmap_last_word_mask(bits) != 0
    {
        return false;
    }
    true
}

/// `bitmap_equal()`: are the first `nbits` bits equal?
pub fn bitmap_equal(src1: &[u64], src2: &[u64], nbits: usize) -> bool {
    __bitmap_equal(src1, src2, nbits)
}

/// `bitmap_or_equal()`: is `src1 | src2 == src3`?
pub fn bitmap_or_equal(src1: &[u64], src2: &[u64], src3: &[u64], nbits: usize) -> bool {
    let lim = nbits / BITS_PER_LONG;
    for k in 0..lim {
        if (src1[k] | src2[k]) != src3[k] {
            return false;
        }
    }
    if nbits % BITS_PER_LONG == 0 {
        return true;
    }
    let tmp = (src1[lim] | src2[lim]) ^ src3[lim];
    tmp & bitmap_last_word_mask(nbits) == 0
}

/// `bitmap_intersects()`: do the bitmaps share a set bit?
pub fn bitmap_intersects(src1: &[u64], src2: &[u64], nbits: usize) -> bool {
    let lim = nbits / BITS_PER_LONG;
    if (0..lim).any(|k| src1[k] & src2[k] != 0) {
        return true;
    }
    nbits % BITS_PER_LONG != 0 && src1[lim] & src2[lim] & bitmap_last_word_mask(nbits) != 0
}

/// `bitmap_subset()`: is every bit of `src1` also set in `src2`?
pub fn bitmap_subset(src1: &[u64], src2: &[u64], nbits: usize) -> bool {
    let lim = nbits / BITS_PER_LONG;
    if (0..lim).any(|k| src1[k] & !src2[k] != 0) {
        return false;
    }
    !(nbits % BITS_PER_LONG != 0 && src1[lim] & !src2[lim] & bitmap_last_word_mask(nbits) != 0)
}

/// `bitmap_empty()`: are all `nbits` bits clear?
pub fn bitmap_empty(src: &[u64], nbits: usize) -> bool {
    find_first_bit(src, nbits) == nbits
}

/// `bitmap_full()`: are all `nbits` bits set?
pub fn bitmap_full(src: &[u64], nbits: usize) -> bool {
    find_first_zero_bit(src, nbits) == nbits
}

/// `bitmap_weight()`: number of set bits among the first `nbits`.
pub fn bitmap_weight(src: &[u64], nbits: usize) -> usize {
    bitmap_weight_by(|i| src[i], nbits)
}

/// `bitmap_weight_and()`: weight of `src1 & src2`.
pub fn bitmap_weight_and(src1: &[u64], src2: &[u64], nbits: usize) -> usize {
    bitmap_weight_by(|i| src1[i] & src2[i], nbits)
}

/// `bitmap_weight_andnot()`: weight of `src1 & !src2`.
pub fn bitmap_weight_andnot(src1: &[u64], src2: &[u64], nbits: usize) -> usize {
    bitmap_weight_by(|i| src1[i] & !src2[i], nbits)
}

/// `bitmap_weight_from()`: number of set bits in `[start, end)`. Returns
/// `end` if `start >= end`.
pub fn bitmap_weight_from(bitmap: &[u64], start: usize, end: usize) -> usize {
    if start >= end {
        return end;
    }
    let bitmap = &bitmap[start / BITS_PER_LONG..];
    // Open-coded round_down(), as in C.
    let end = end - (start & !(BITS_PER_LONG - 1));
    let start = start % BITS_PER_LONG;
    let mut w = bitmap_weight(bitmap, end);
    if start != 0 {
        w -= (bitmap[0] & bitmap_last_word_mask(start)).count_ones() as usize;
    }
    w
}

/// `__bitmap_set()`: sets `len` bits from `start`.
pub fn __bitmap_set(map: &mut [u64], start: usize, mut len: usize) {
    let mut p = bit_word(start);
    let size = start + len;
    let mut bits_to_set = BITS_PER_LONG - (start % BITS_PER_LONG);
    let mut mask_to_set = bitmap_first_word_mask(start);

    while len >= bits_to_set {
        map[p] |= mask_to_set;
        len -= bits_to_set;
        bits_to_set = BITS_PER_LONG;
        mask_to_set = !0;
        p += 1;
    }
    if len != 0 {
        mask_to_set &= bitmap_last_word_mask(size);
        map[p] |= mask_to_set;
    }
}

/// `__bitmap_clear()`: clears `len` bits from `start`.
pub fn __bitmap_clear(map: &mut [u64], start: usize, mut len: usize) {
    let mut p = bit_word(start);
    let size = start + len;
    let mut bits_to_clear = BITS_PER_LONG - (start % BITS_PER_LONG);
    let mut mask_to_clear = bitmap_first_word_mask(start);

    while len >= bits_to_clear {
        map[p] &= !mask_to_clear;
        len -= bits_to_clear;
        bits_to_clear = BITS_PER_LONG;
        mask_to_clear = !0;
        p += 1;
    }
    if len != 0 {
        mask_to_clear &= bitmap_last_word_mask(size);
        map[p] &= !mask_to_clear;
    }
}

/// `bitmap_set()`: sets `nbits` bits from `start`.
pub fn bitmap_set(map: &mut [u64], start: usize, nbits: usize) {
    __bitmap_set(map, start, nbits);
}

/// `bitmap_clear()`: clears `nbits` bits from `start`.
pub fn bitmap_clear(map: &mut [u64], start: usize, nbits: usize) {
    __bitmap_clear(map, start, nbits);
}

/// `bitmap_shift_right()`: `dst = src >> shift`, zero-filling from the top.
pub fn bitmap_shift_right(dst: &mut [u64], src: &[u64], shift: usize, nbits: usize) {
    let lim = bits_to_longs(nbits);
    let off = shift / BITS_PER_LONG;
    let rem = shift % BITS_PER_LONG;
    let mask = bitmap_last_word_mask(nbits);
    let mut k = 0;
    while off + k < lim {
        // If shift is not word aligned, take lower rem bits of word above
        // and make them the top rem bits of result.
        let upper = if rem == 0 || off + k + 1 >= lim {
            0
        } else {
            let mut upper = src[off + k + 1];
            if off + k + 1 == lim - 1 {
                upper &= mask;
            }
            upper << (BITS_PER_LONG - rem)
        };
        let mut lower = src[off + k];
        if off + k == lim - 1 {
            lower &= mask;
        }
        lower >>= rem;
        dst[k] = lower | upper;
        k += 1;
    }
    if off != 0 {
        dst[lim.saturating_sub(off)..lim].fill(0);
    }
}

/// `bitmap_shift_left()`: `dst = src << shift`, zero-filling from the
/// bottom. Bits above `nbits` in the last word are left as they come out.
///
/// For shifts of `nbits` or more C writes zeros past the bitmap; here the
/// whole bitmap becomes zero.
pub fn bitmap_shift_left(dst: &mut [u64], src: &[u64], shift: usize, nbits: usize) {
    let lim = bits_to_longs(nbits);
    let off = shift / BITS_PER_LONG;
    let rem = shift % BITS_PER_LONG;
    for k in (0..lim.saturating_sub(off)).rev() {
        // If shift is not word aligned, take upper rem bits of word below
        // and make them the bottom rem bits of result.
        let lower = if rem != 0 && k > 0 {
            src[k - 1] >> (BITS_PER_LONG - rem)
        } else {
            0
        };
        let upper = src[k] << rem;
        dst[k + off] = lower | upper;
    }
    if off != 0 {
        dst[..off.min(lim)].fill(0);
    }
}

/// `bitmap_cut()`: removes `cut` bits starting at `first` and shifts the
/// bits above them down. `dst` is a copy of `src` with the region removed.
pub fn bitmap_cut(dst: &mut [u64], src: &[u64], first: usize, cut: usize, nbits: usize) {
    let len = bits_to_longs(nbits);
    dst[..len].copy_from_slice(&src[..len]);
    bitmap_cut_in_place(dst, first, cut, nbits);
}

/// [`bitmap_cut`] on a single bitmap, for the C callers that pass
/// overlapping `dst` and `src`: `memmove()` the source into place with
/// `copy_within()` first.
pub fn bitmap_cut_in_place(dst: &mut [u64], first: usize, mut cut: usize, nbits: usize) {
    let len = bits_to_longs(nbits);
    let fw = first / BITS_PER_LONG;
    let keep = if first % BITS_PER_LONG != 0 && fw < len {
        dst[fw] & (!0u64 >> (BITS_PER_LONG - first % BITS_PER_LONG))
    } else {
        0
    };

    while cut > 0 {
        cut -= 1;
        for i in fw..len {
            let carry = if i < len - 1 { dst[i + 1] & 1 } else { 0 };
            dst[i] = (dst[i] >> 1) | (carry << (BITS_PER_LONG - 1));
        }
    }

    // C touches the word holding `first` even when it is past the bitmap;
    // that write changes nothing, so it is skipped here.
    if fw < len {
        dst[fw] &= !0u64 << (first % BITS_PER_LONG);
        dst[fw] |= keep;
    }
}

/// `bitmap_replace()`: `dst = (old & !mask) | (new & mask)`.
pub fn bitmap_replace(dst: &mut [u64], old: &[u64], new: &[u64], mask: &[u64], nbits: usize) {
    for k in 0..bits_to_longs(nbits) {
        dst[k] = (old[k] & !mask[k]) | (new[k] & mask[k]);
    }
}

/// `bitmap_scatter()`: copies the low bits of `src`, in order, to the
/// positions of the set bits of `mask`.
///
/// If `src` = 0x005a and `mask` = 0x1313, `dst` becomes 0x0302.
pub fn bitmap_scatter(dst: &mut [u64], src: &[u64], mask: &[u64], nbits: usize) {
    bitmap_zero(dst, nbits);
    for (n, bit) in for_each_set_bit(mask, nbits).enumerate() {
        assign_bit(bit, dst, test_bit(n, src));
    }
}

/// `bitmap_gather()`: the reverse of [`bitmap_scatter`]: packs the bits of
/// `src` at the set positions of `mask` into the low bits of `dst`.
///
/// If `src` = 0x0302 and `mask` = 0x1313, `dst` becomes 0x001a.
pub fn bitmap_gather(dst: &mut [u64], src: &[u64], mask: &[u64], nbits: usize) {
    bitmap_zero(dst, nbits);
    for (n, bit) in for_each_set_bit(mask, nbits).enumerate() {
        assign_bit(n, dst, test_bit(bit, src));
    }
}

/// `bitmap_release_region()`: clears the `2^order` bits at `pos`.
pub fn bitmap_release_region(bitmap: &mut [u64], pos: usize, order: u32) {
    bitmap_clear(bitmap, pos, 1 << order);
}

/// `bitmap_allocate_region()`: sets the `2^order` bits at `pos` if they are
/// all clear, else returns `EBUSY`.
pub fn bitmap_allocate_region(bitmap: &mut [u64], pos: usize, order: u32) -> Result {
    let len = 1usize << order;
    if find_next_bit(bitmap, pos + len, pos) < pos + len {
        return Err(EBUSY);
    }
    bitmap_set(bitmap, pos, len);
    Ok(())
}

/// `bitmap_find_free_region()`: finds and allocates a clear region of
/// `2^order` bits aligned to its size. Returns its position or `ENOMEM`.
pub fn bitmap_find_free_region(bitmap: &mut [u64], bits: usize, order: u32) -> Result<usize> {
    let mut pos = 0;
    loop {
        let end = pos + (1usize << order);
        if end > bits {
            return Err(ENOMEM);
        }
        if bitmap_allocate_region(bitmap, pos, order).is_ok() {
            return Ok(pos);
        }
        pos = end;
    }
}

/// `bitmap_read()`: the `nbits`-bit value at bit `start`. Returns 0 for
/// `nbits` of 0 or more than 64.
pub fn bitmap_read(map: &[u64], start: usize, nbits: usize) -> u64 {
    if nbits == 0 || nbits > BITS_PER_LONG {
        return 0;
    }
    let index = bit_word(start);
    let offset = start % BITS_PER_LONG;
    let space = BITS_PER_LONG - offset;

    if space >= nbits {
        return (map[index] >> offset) & bitmap_last_word_mask(nbits);
    }

    let value_low = map[index] & bitmap_first_word_mask(start);
    let value_high = map[index + 1] & bitmap_last_word_mask(start + nbits);
    (value_low >> offset) | (value_high << space)
}

/// `bitmap_write()`: stores the low `nbits` bits of `value` at bit `start`.
/// Does nothing for `nbits` of 0 or more than 64.
pub fn bitmap_write(map: &mut [u64], mut value: u64, start: usize, nbits: usize) {
    if nbits == 0 || nbits > BITS_PER_LONG {
        return;
    }
    let mask = bitmap_last_word_mask(nbits);
    value &= mask;
    let offset = start % BITS_PER_LONG;
    let space = BITS_PER_LONG - offset;
    let fit = space >= nbits;
    let index = bit_word(start);

    map[index] &= if fit {
        !(mask << offset)
    } else {
        !bitmap_first_word_mask(start)
    };
    map[index] |= value << offset;
    if fit {
        return;
    }

    map[index + 1] &= bitmap_first_word_mask(start + nbits);
    map[index + 1] |= value >> space;
}

/// `bitmap_get_value8()`: the byte at bit `start`.
pub fn bitmap_get_value8(map: &[u64], start: usize) -> u64 {
    bitmap_read(map, start, 8)
}

/// `bitmap_set_value8()`: stores a byte at bit `start`.
pub fn bitmap_set_value8(map: &mut [u64], value: u64, start: usize) {
    bitmap_write(map, value, start, 8);
}

/// `bitmap_find_next_zero_area_off()`: first run of `nr` clear bits at or
/// after `start` whose position plus `align_offset` is a multiple of
/// `align_mask + 1`. Returns its position or a value `>= size`.
pub fn bitmap_find_next_zero_area_off(
    map: &[u64],
    size: usize,
    start: usize,
    nr: usize,
    align_mask: usize,
    align_offset: usize,
) -> usize {
    let mut start = start;
    loop {
        // for_each_clear_bit_from(start, map, size)
        start = find_next_zero_bit(map, size, start);
        if start >= size {
            break;
        }
        // __ALIGN_MASK()
        start = ((start + align_offset + align_mask) & !align_mask) - align_offset;
        let end = start + nr;
        if end > size {
            break;
        }
        let off = start & !(BITS_PER_LONG - 1);
        let i = find_last_bit(&map[start / BITS_PER_LONG..], end - off) + off;
        if i >= end || i < start {
            return start;
        }
        start = i + 1;
    }
    size
}

/// `bitmap_find_next_zero_area()`: [`bitmap_find_next_zero_area_off`]
/// without an offset.
pub fn bitmap_find_next_zero_area(
    map: &[u64],
    size: usize,
    start: usize,
    nr: usize,
    align_mask: usize,
) -> usize {
    bitmap_find_next_zero_area_off(map, size, start, nr, align_mask, 0)
}

/// `bitmap_pos_to_ord()`: which set bit, counting from 0, is bit `pos`?
/// -1 if it is clear or out of range.
fn bitmap_pos_to_ord(buf: &[u64], pos: usize, nbits: usize) -> isize {
    if pos >= nbits || !test_bit(pos, buf) {
        return -1;
    }
    bitmap_weight(buf, pos) as isize
}

/// `bitmap_remap()`: maps `src` through the map that sends the n-th set
/// bit of `old` to the (n mod w)-th set bit of `new`, `w` being the weight
/// of `new`. Bits that are clear in `old` map to themselves.
pub fn bitmap_remap(dst: &mut [u64], src: &[u64], old: &[u64], new: &[u64], nbits: usize) {
    bitmap_zero(dst, nbits);
    let w = bitmap_weight(new, nbits);
    for oldbit in for_each_set_bit(src, nbits) {
        let n = bitmap_pos_to_ord(old, oldbit, nbits);
        if n < 0 || w == 0 {
            set_bit(oldbit, dst); // identity map
        } else {
            set_bit(find_nth_bit(new, nbits, n as usize % w), dst);
        }
    }
}

/// `bitmap_bitremap()`: [`bitmap_remap`] for a single bit.
pub fn bitmap_bitremap(oldbit: usize, old: &[u64], new: &[u64], bits: usize) -> usize {
    let w = bitmap_weight(new, bits);
    let n = bitmap_pos_to_ord(old, oldbit, bits);
    if n < 0 || w == 0 {
        oldbit
    } else {
        find_nth_bit(new, bits, n as usize % w)
    }
}

/// `bitmap_onto()`: sets bit n of `dst` if bit n of `relmap` is its m-th
/// set bit and bit m of `orig` is set.
pub fn bitmap_onto(dst: &mut [u64], orig: &[u64], relmap: &[u64], bits: usize) {
    bitmap_zero(dst, bits);
    for (m, n) in for_each_set_bit(relmap, bits).enumerate() {
        if test_bit(m, orig) {
            set_bit(n, dst);
        }
    }
}

/// `bitmap_fold()`: sets bit `m % sz` of `dst` for every bit m of `orig`.
pub fn bitmap_fold(dst: &mut [u64], orig: &[u64], sz: usize, nbits: usize) {
    bitmap_zero(dst, nbits);
    for oldbit in for_each_set_bit(orig, nbits) {
        set_bit(oldbit % sz, dst);
    }
}

// ---- lib/bitmap-str.c ----

/// `bitmap_print_bitmask_to_buf()`: copies the part of `"%*pb\n"` (with
/// its NUL) that starts at byte `off` into `buf`. Returns the number of
/// bytes copied.
pub fn bitmap_print_bitmask_to_buf(
    buf: &mut [u8],
    maskp: &[u64],
    nmaskbits: usize,
    off: usize,
) -> usize {
    crate::vsprintf::bitmap_print_window(false, buf, maskp, nmaskbits, off)
}

/// `bitmap_print_list_to_buf()`: like [`bitmap_print_bitmask_to_buf`] with
/// the `"%*pbl\n"` list format.
pub fn bitmap_print_list_to_buf(
    buf: &mut [u8],
    maskp: &[u64],
    nmaskbits: usize,
    off: usize,
) -> usize {
    crate::vsprintf::bitmap_print_window(true, buf, maskp, nmaskbits, off)
}

/// A parsed region `start-end:off/group_len`.
///
/// Region 9-38:4/10 describes the following bitmap structure:
///
/// ```text
/// 0      9  12    18                  38           N
/// .........****......****......****..................
///          ^  ^     ^                 ^            ^
///      start  off   group_len        end         nbits
/// ```
struct Region {
    start: u32,
    off: u32,
    group_len: u32,
    end: u32,
    nbits: u32,
}

fn bitmap_set_region(r: &Region, bitmap: &mut [u64]) {
    let mut start = r.start as usize;
    while start <= r.end as usize {
        bitmap_set(
            bitmap,
            start,
            (r.end as usize - start + 1).min(r.off as usize),
        );
        start += r.group_len as usize;
    }
}

fn bitmap_check_region(r: &Region) -> Result {
    if r.start > r.end || r.group_len == 0 || r.off > r.group_len {
        return Err(EINVAL);
    }
    if r.end >= r.nbits {
        return Err(ERANGE);
    }
    Ok(())
}

/// Parses a number or `N` at `pos`; returns the number and the index after it.
fn bitmap_getnum(s: &[u8], pos: usize, lastbit: u32) -> Result<(u32, usize)> {
    if at(s, pos) == b'N' {
        return Ok((lastbit, pos + 1));
    }
    let (len, n) = parse_integer(s.get(pos..).unwrap_or(&[]), 10);
    if len == 0 {
        return Err(EINVAL);
    }
    if len & KSTRTOX_OVERFLOW != 0 || n != n as u32 as u64 {
        return Err(EOVERFLOW);
    }
    Ok((n as u32, pos + len as usize))
}

#[inline]
fn end_of_str(c: u8) -> bool {
    c == 0 || c == b'\n'
}

#[inline]
fn __end_of_region(c: u8) -> bool {
    isspace(c) || c == b','
}

#[inline]
fn end_of_region(c: u8) -> bool {
    __end_of_region(c) || end_of_str(c)
}

/// The format allows commas and whitespace at the beginning of the region.
fn bitmap_find_region(s: &[u8], mut pos: usize) -> Option<usize> {
    while __end_of_region(at(s, pos)) {
        pos += 1;
    }
    if end_of_str(at(s, pos)) {
        None
    } else {
        Some(pos)
    }
}

/// Parses one region at `pos`. Returns where parsing continues, or `None`
/// at the end of the string.
fn bitmap_parse_region(s: &[u8], mut pos: usize, r: &mut Region) -> Result<Option<usize>> {
    let lastbit = r.nbits.wrapping_sub(1);

    let pattern = if strncasecmp(s.get(pos..).unwrap_or(&[]), b"all", 3) == 0 {
        r.start = 0;
        r.end = lastbit;
        pos += 3;
        true
    } else {
        (r.start, pos) = bitmap_getnum(s, pos, lastbit)?;
        if end_of_region(at(s, pos)) {
            // no_end:
            r.end = r.start;
            false
        } else {
            if at(s, pos) != b'-' {
                return Err(EINVAL);
            }
            (r.end, pos) = bitmap_getnum(s, pos + 1, lastbit)?;
            true
        }
    };

    if pattern && !end_of_region(at(s, pos)) {
        // check_pattern:
        if at(s, pos) != b':' {
            return Err(EINVAL);
        }
        (r.off, pos) = bitmap_getnum(s, pos + 1, lastbit)?;
        if at(s, pos) != b'/' {
            return Err(EINVAL);
        }
        (r.group_len, pos) = bitmap_getnum(s, pos + 1, lastbit)?;
        return Ok(Some(pos));
    }

    // no_pattern:
    r.off = r.end.wrapping_add(1);
    r.group_len = r.end.wrapping_add(1);
    Ok(if end_of_str(at(s, pos)) {
        None
    } else {
        Some(pos)
    })
}

/// `bitmap_parselist()`: parses a list such as `"0-3,8,16-31:2/4"` into a
/// bitmap of `nmaskbits` bits.
///
/// The list is comma-separated decimal bit numbers and ranges `a-b`. A range
/// may end in `:used/group`: only the first `used` bits of each `group`
/// bits are set, so `0-1023:2/256` sets 0, 1, 256, 257, 512, 513, 768 and
/// 769. `N` stands for the last bit, `nmaskbits - 1`, and `all` for `0-N`.
///
/// Errors: `EINVAL` for a malformed region or an invalid character,
/// `ERANGE` for a bit number too large for the bitmap and `EOVERFLOW` for
/// a number that does not fit in 32 bits.
pub fn bitmap_parselist(buf: &[u8], maskp: &mut [u64], nmaskbits: usize) -> Result {
    let mut r = Region {
        start: 0,
        off: 0,
        group_len: 0,
        end: 0,
        nbits: nmaskbits as u32,
    };
    bitmap_zero(maskp, nmaskbits);

    let mut pos = Some(0);
    while let Some(p) = pos {
        let Some(p) = bitmap_find_region(buf, p) else {
            return Ok(());
        };
        pos = bitmap_parse_region(buf, p, &mut r)?;
        bitmap_check_region(&r)?;
        bitmap_set_region(&r, maskp);
    }
    Ok(())
}

/// Skips region separators backwards from `end`.
fn bitmap_find_region_reverse(s: &[u8], end: isize) -> isize {
    let mut end = end;
    while 0 <= end && __end_of_region(at(s, end as usize)) {
        end -= 1;
    }
    end
}

/// Reads up to eight hex digits backwards from `end` into `num`. Returns
/// the index before them.
fn bitmap_get_x32_reverse(s: &[u8], mut end: isize, num: &mut u32) -> Result<isize> {
    let mut ret = 0u32;
    let mut i = 0;
    while i < 32 {
        let c = hex_to_bin(at(s, end as usize));
        end -= 1;
        if c < 0 {
            return Err(EINVAL);
        }
        ret |= (c as u32) << i;
        if 0 > end || __end_of_region(at(s, end as usize)) {
            *num = ret;
            return Ok(end);
        }
        i += 4;
    }
    if hex_to_bin(at(s, end as usize)) >= 0 {
        return Err(EOVERFLOW);
    }
    *num = ret;
    Ok(end)
}

/// Stores 32-bit chunk `chunk` of a bitmap, as C does through a `u32 *`
/// on a little-endian machine.
fn set_chunk32(maskp: &mut [u64], chunk: usize, val: u32) {
    let word = &mut maskp[chunk / 2];
    let shift = (chunk % 2) * 32;
    *word = (*word & !(0xffff_ffffu64 << shift)) | ((val as u64) << shift);
}

/// `bitmap_parse()`: parses a hex mask such as `"deadbeef,1,0"` into a
/// bitmap of `nmaskbits` bits.
///
/// The string ends at a NUL, a newline or after `buflen` bytes. Commas
/// group hex digits into 32-bit chunks, most significant first; a chunk
/// with fewer digits gets leading zeros. Grouping such as `"1,,5"`, `",44"`,
/// `","` or `""` is allowed, as is white space around chunks.
///
/// Errors: `EINVAL` for an invalid character, `EOVERFLOW` for a chunk of
/// more than 32 bits or a value that does not fit in `nmaskbits`.
pub fn bitmap_parse(start: &[u8], buflen: usize, maskp: &mut [u64], nmaskbits: usize) -> Result {
    let mut end = strnchrnul(start, buflen, b'\n') as isize - 1;
    let mut chunks = bits_to_u32(nmaskbits);

    let mut chunk = 0;
    loop {
        end = bitmap_find_region_reverse(start, end);
        if 0 > end {
            break;
        }
        if chunks == 0 {
            return Err(EOVERFLOW);
        }
        chunks -= 1;
        let mut val = 0;
        end = bitmap_get_x32_reverse(start, end, &mut val)?;
        set_chunk32(maskp, chunk, val);
        chunk += 1;
    }

    let unset_bit = (bits_to_u32(nmaskbits) - chunks) * 32;
    if unset_bit < nmaskbits {
        bitmap_clear(maskp, unset_bit, nmaskbits - unset_bit);
        return Ok(());
    }

    if find_next_bit(maskp, unset_bit, nmaskbits) != unset_bit {
        return Err(EOVERFLOW);
    }
    Ok(())
}

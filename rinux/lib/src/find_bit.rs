// SPDX-License-Identifier: GPL-2.0-or-later
//! Searching bitmaps for set and clear bits.
//!
//! Ported from `lib/find_bit.c` and `include/linux/find.h`.
//!
//! Copyright (C) 2004 Red Hat, Inc. All Rights Reserved.
//! Written by David Howells (dhowells@redhat.com)
//!
//! Copyright (C) 2008 IBM Corporation
//! 'find_last_bit' is written by Rusty Russell <rusty@rustcorp.com.au>
//! (Inspired by David Howell's find_next_bit implementation)
//!
//! Rewritten by Yury Norov <yury.norov@gmail.com> to decrease
//! size and improve performance, 2015.
//!
//! Bitmaps are `[u64]` slices as described in [`crate::bitops`]. Every
//! search returns the bit number it found, or `size` when there is none.
//! Only the words that hold bits below `size` are read, so a zero-sized
//! search on an empty slice is fine.
//!
//! The `for_each_*` macros of `find.h` are iterators here. The C header's
//! fast paths for bitmaps of one word known at compile time give the same
//! results as the general code and are not ported.
//!
//! `find_random_bit()` needs `get_random_u32_below()` and will come with the
//! random number generator.

use crate::bitmap::{bitmap_first_word_mask, bitmap_last_word_mask, bitmap_read};
use crate::bitops::{__ffs, __fls, BITS_PER_LONG, fns};

/// `FIND_FIRST_BIT()`: first set bit of the words produced by `fetch`.
#[inline]
fn find_first(fetch: impl Fn(usize) -> u64, size: usize) -> usize {
    let mut idx = 0;
    while idx * BITS_PER_LONG < size {
        let val = fetch(idx);
        if val != 0 {
            return (idx * BITS_PER_LONG + __ffs(val) as usize).min(size);
        }
        idx += 1;
    }
    size
}

/// `FIND_NEXT_BIT()`: next set bit at or after `start`.
#[inline]
fn find_next(fetch: impl Fn(usize) -> u64, size: usize, start: usize) -> usize {
    if start >= size {
        return size;
    }
    let mask = bitmap_first_word_mask(start);
    let mut idx = start / BITS_PER_LONG;
    let mut tmp = fetch(idx) & mask;
    while tmp == 0 {
        if (idx + 1) * BITS_PER_LONG >= size {
            return size;
        }
        idx += 1;
        tmp = fetch(idx);
    }
    (idx * BITS_PER_LONG + __ffs(tmp) as usize).min(size)
}

/// `FIND_NTH_BIT()`: the `num`-th set bit, counting from 0. Returns a value
/// `>= size` if there is none.
#[inline]
fn find_nth(fetch: impl Fn(usize) -> u64, size: usize, num: usize) -> usize {
    let mut nr = num;
    let mut idx = 0;
    let mut tmp = 0;
    while (idx + 1) * BITS_PER_LONG <= size {
        if idx * BITS_PER_LONG + nr >= size {
            return size;
        }
        tmp = fetch(idx);
        let w = tmp.count_ones() as usize;
        if w > nr {
            return idx * BITS_PER_LONG + fns(tmp, nr as u32) as usize;
        }
        nr -= w;
        idx += 1;
    }
    if size % BITS_PER_LONG != 0 {
        tmp = fetch(idx) & bitmap_last_word_mask(size);
    }
    idx * BITS_PER_LONG + fns(tmp, nr.min(u32::MAX as usize) as u32) as usize
}

/// `find_first_bit()`: first set bit.
pub fn find_first_bit(addr: &[u64], size: usize) -> usize {
    find_first(|i| addr[i], size)
}

/// `find_first_and_bit()`: first bit set in both bitmaps.
pub fn find_first_and_bit(addr1: &[u64], addr2: &[u64], size: usize) -> usize {
    find_first(|i| addr1[i] & addr2[i], size)
}

/// `find_first_andnot_bit()`: first bit set in `addr1` and clear in `addr2`.
pub fn find_first_andnot_bit(addr1: &[u64], addr2: &[u64], size: usize) -> usize {
    find_first(|i| addr1[i] & !addr2[i], size)
}

/// `find_first_and_and_bit()`: first bit set in all three bitmaps.
pub fn find_first_and_and_bit(addr1: &[u64], addr2: &[u64], addr3: &[u64], size: usize) -> usize {
    find_first(|i| addr1[i] & addr2[i] & addr3[i], size)
}

/// `find_first_zero_bit()`: first clear bit.
pub fn find_first_zero_bit(addr: &[u64], size: usize) -> usize {
    find_first(|i| !addr[i], size)
}

/// `find_next_bit()`: next set bit at or after `offset`.
pub fn find_next_bit(addr: &[u64], size: usize, offset: usize) -> usize {
    find_next(|i| addr[i], size, offset)
}

/// `find_next_and_bit()`: next bit set in both bitmaps.
pub fn find_next_and_bit(addr1: &[u64], addr2: &[u64], size: usize, offset: usize) -> usize {
    find_next(|i| addr1[i] & addr2[i], size, offset)
}

/// `find_next_andnot_bit()`: next bit set in `addr1` and clear in `addr2`.
pub fn find_next_andnot_bit(addr1: &[u64], addr2: &[u64], size: usize, offset: usize) -> usize {
    find_next(|i| addr1[i] & !addr2[i], size, offset)
}

/// `find_next_or_bit()`: next bit set in either bitmap.
pub fn find_next_or_bit(addr1: &[u64], addr2: &[u64], size: usize, offset: usize) -> usize {
    find_next(|i| addr1[i] | addr2[i], size, offset)
}

/// `find_next_zero_bit()`: next clear bit at or after `offset`.
pub fn find_next_zero_bit(addr: &[u64], size: usize, offset: usize) -> usize {
    find_next(|i| !addr[i], size, offset)
}

/// `find_nth_bit()`: the `n`-th set bit, counting from 0, or a value
/// `>= size` if there are not that many.
pub fn find_nth_bit(addr: &[u64], size: usize, n: usize) -> usize {
    if n >= size {
        return size;
    }
    find_nth(|i| addr[i], size, n)
}

/// `find_nth_and_bit()`: [`find_nth_bit`] in `addr1 & addr2`.
pub fn find_nth_and_bit(addr1: &[u64], addr2: &[u64], size: usize, n: usize) -> usize {
    if n >= size {
        return size;
    }
    find_nth(|i| addr1[i] & addr2[i], size, n)
}

/// `find_nth_and_andnot_bit()`: [`find_nth_bit`] in `addr1 & addr2 & !addr3`.
pub fn find_nth_and_andnot_bit(
    addr1: &[u64],
    addr2: &[u64],
    addr3: &[u64],
    size: usize,
    n: usize,
) -> usize {
    if n >= size {
        return size;
    }
    find_nth(|i| addr1[i] & addr2[i] & !addr3[i], size, n)
}

/// `find_last_bit()`: last set bit.
pub fn find_last_bit(addr: &[u64], size: usize) -> usize {
    if size != 0 {
        let mut val = bitmap_last_word_mask(size);
        let mut idx = (size - 1) / BITS_PER_LONG;
        loop {
            val &= addr[idx];
            if val != 0 {
                return idx * BITS_PER_LONG + __fls(val) as usize;
            }
            val = !0;
            if idx == 0 {
                break;
            }
            idx -= 1;
        }
    }
    size
}

/// `find_next_and_bit_wrap()`: next bit set in both bitmaps, wrapping
/// around to the start once.
pub fn find_next_and_bit_wrap(addr1: &[u64], addr2: &[u64], size: usize, offset: usize) -> usize {
    let bit = find_next_and_bit(addr1, addr2, size, offset);
    if bit < size || offset == 0 {
        return bit;
    }
    let bit = find_first_and_bit(addr1, addr2, offset);
    if bit < offset { bit } else { size }
}

/// `find_next_bit_wrap()`: next set bit, wrapping around to the start once.
pub fn find_next_bit_wrap(addr: &[u64], size: usize, offset: usize) -> usize {
    let bit = find_next_bit(addr, size, offset);
    if bit < size || offset == 0 {
        return bit;
    }
    let bit = find_first_bit(addr, offset);
    if bit < offset { bit } else { size }
}

/// `__for_each_wrap()`: the step of [`for_each_set_bit_wrap`].
pub fn __for_each_wrap(bitmap: &[u64], size: usize, start: usize, mut n: usize) -> usize {
    // If not wrapped around...
    if n > start {
        // ...and have a bit, just return it.
        let bit = find_next_bit(bitmap, size, n);
        if bit < size {
            return bit;
        }
        // Otherwise, wrap around and...
        n = 0;
    }
    // ...search the other part.
    let bit = find_next_bit(bitmap, start, n);
    if bit < start { bit } else { size }
}

/// `find_next_clump8()`: next 8-bit clump, aligned to 8 bits, that has a set
/// bit at or after `offset`. Stores the clump in `clump` and returns its
/// offset, or returns `size` without touching `clump`.
pub fn find_next_clump8(clump: &mut u64, addr: &[u64], size: usize, offset: usize) -> usize {
    let offset = find_next_bit(addr, size, offset);
    if offset == size {
        return size;
    }
    let offset = offset & !7;
    *clump = bitmap_read(addr, offset, 8);
    offset
}

/// `find_first_clump8()`.
pub fn find_first_clump8(clump: &mut u64, bits: &[u64], size: usize) -> usize {
    find_next_clump8(clump, bits, size, 0)
}

/// `find_first_zero_bit_le()`: rinux targets are little-endian, so this is
/// [`find_first_zero_bit`].
pub fn find_first_zero_bit_le(addr: &[u64], size: usize) -> usize {
    find_first_zero_bit(addr, size)
}

/// `find_next_zero_bit_le()`: [`find_next_zero_bit`] on little-endian.
pub fn find_next_zero_bit_le(addr: &[u64], size: usize, offset: usize) -> usize {
    find_next_zero_bit(addr, size, offset)
}

/// `find_next_bit_le()`: [`find_next_bit`] on little-endian.
pub fn find_next_bit_le(addr: &[u64], size: usize, offset: usize) -> usize {
    find_next_bit(addr, size, offset)
}

/// Iterator behind the `for_each_*_bit` loops: calls `next(bit)` to find
/// each bit at or after `bit` until it returns `size`.
pub struct BitIter<F> {
    bit: usize,
    size: usize,
    next: F,
}

impl<F: Fn(usize) -> usize> Iterator for BitIter<F> {
    type Item = usize;

    fn next(&mut self) -> Option<usize> {
        if self.bit >= self.size {
            return None;
        }
        self.bit = (self.next)(self.bit);
        if self.bit >= self.size {
            return None;
        }
        let bit = self.bit;
        self.bit += 1;
        Some(bit)
    }
}

/// `for_each_set_bit()`: the set bits below `size`, in order.
pub fn for_each_set_bit(addr: &[u64], size: usize) -> BitIter<impl Fn(usize) -> usize + '_> {
    for_each_set_bit_from(0, addr, size)
}

/// `for_each_set_bit_from()`: the set bits from `bit` up to `size`.
pub fn for_each_set_bit_from(
    bit: usize,
    addr: &[u64],
    size: usize,
) -> BitIter<impl Fn(usize) -> usize + '_> {
    BitIter {
        bit,
        size,
        next: move |b| find_next_bit(addr, size, b),
    }
}

/// `for_each_clear_bit()`: the clear bits below `size`, in order.
pub fn for_each_clear_bit(addr: &[u64], size: usize) -> BitIter<impl Fn(usize) -> usize + '_> {
    for_each_clear_bit_from(0, addr, size)
}

/// `for_each_clear_bit_from()`: the clear bits from `bit` up to `size`.
pub fn for_each_clear_bit_from(
    bit: usize,
    addr: &[u64],
    size: usize,
) -> BitIter<impl Fn(usize) -> usize + '_> {
    BitIter {
        bit,
        size,
        next: move |b| find_next_zero_bit(addr, size, b),
    }
}

/// `for_each_and_bit()`: the bits set in both bitmaps.
pub fn for_each_and_bit<'a>(
    addr1: &'a [u64],
    addr2: &'a [u64],
    size: usize,
) -> BitIter<impl Fn(usize) -> usize + 'a> {
    BitIter {
        bit: 0,
        size,
        next: move |b| find_next_and_bit(addr1, addr2, size, b),
    }
}

/// `for_each_andnot_bit()`: the bits set in `addr1` and clear in `addr2`.
pub fn for_each_andnot_bit<'a>(
    addr1: &'a [u64],
    addr2: &'a [u64],
    size: usize,
) -> BitIter<impl Fn(usize) -> usize + 'a> {
    BitIter {
        bit: 0,
        size,
        next: move |b| find_next_andnot_bit(addr1, addr2, size, b),
    }
}

/// `for_each_or_bit()`: the bits set in either bitmap.
pub fn for_each_or_bit<'a>(
    addr1: &'a [u64],
    addr2: &'a [u64],
    size: usize,
) -> BitIter<impl Fn(usize) -> usize + 'a> {
    BitIter {
        bit: 0,
        size,
        next: move |b| find_next_or_bit(addr1, addr2, size, b),
    }
}

/// Iterator behind the `for_each_*_bitrange` loops. Yields `(b, e)`: the
/// range `[b, e)` is a maximal run of set (or clear) bits.
pub struct RangeIter<'a> {
    addr: &'a [u64],
    size: usize,
    b: usize,
    set: bool,
}

impl Iterator for RangeIter<'_> {
    type Item = (usize, usize);

    fn next(&mut self) -> Option<(usize, usize)> {
        let (b, e) = if self.set {
            let b = find_next_bit(self.addr, self.size, self.b);
            (b, find_next_zero_bit(self.addr, self.size, b + 1))
        } else {
            let b = find_next_zero_bit(self.addr, self.size, self.b);
            (b, find_next_bit(self.addr, self.size, b + 1))
        };
        if b >= self.size {
            self.b = self.size;
            return None;
        }
        self.b = e + 1;
        Some((b, e))
    }
}

/// `for_each_set_bitrange()`: the runs of set bits as `[b, e)` ranges.
pub fn for_each_set_bitrange(addr: &[u64], size: usize) -> RangeIter<'_> {
    for_each_set_bitrange_from(0, addr, size)
}

/// `for_each_set_bitrange_from()`: [`for_each_set_bitrange`] from bit `b`.
pub fn for_each_set_bitrange_from(b: usize, addr: &[u64], size: usize) -> RangeIter<'_> {
    RangeIter {
        addr,
        size,
        b,
        set: true,
    }
}

/// `for_each_clear_bitrange()`: the runs of clear bits as `[b, e)` ranges.
pub fn for_each_clear_bitrange(addr: &[u64], size: usize) -> RangeIter<'_> {
    for_each_clear_bitrange_from(0, addr, size)
}

/// `for_each_clear_bitrange_from()`: [`for_each_clear_bitrange`] from bit `b`.
pub fn for_each_clear_bitrange_from(b: usize, addr: &[u64], size: usize) -> RangeIter<'_> {
    RangeIter {
        addr,
        size,
        b,
        set: false,
    }
}

/// Iterator returned by [`for_each_set_bit_wrap`].
pub struct WrapIter<'a> {
    addr: &'a [u64],
    size: usize,
    start: usize,
    bit: usize,
}

impl Iterator for WrapIter<'_> {
    type Item = usize;

    fn next(&mut self) -> Option<usize> {
        if self.bit >= self.size {
            return None;
        }
        let bit = self.bit;
        self.bit = __for_each_wrap(self.addr, self.size, self.start, bit + 1);
        Some(bit)
    }
}

/// `for_each_set_bit_wrap()`: the set bits from `start` to the end, then
/// from 0 up to `start`.
pub fn for_each_set_bit_wrap(addr: &[u64], size: usize, start: usize) -> WrapIter<'_> {
    WrapIter {
        addr,
        size,
        start,
        bit: find_next_bit_wrap(addr, size, start),
    }
}

/// Iterator returned by [`for_each_set_clump8`].
pub struct Clump8Iter<'a> {
    bits: &'a [u64],
    size: usize,
    start: usize,
    clump: u64,
}

impl Iterator for Clump8Iter<'_> {
    type Item = (usize, u64);

    fn next(&mut self) -> Option<(usize, u64)> {
        if self.start >= self.size {
            return None;
        }
        let item = (self.start, self.clump);
        self.start = find_next_clump8(&mut self.clump, self.bits, self.size, self.start + 8);
        Some(item)
    }
}

/// `for_each_set_clump8()`: each 8-bit aligned clump with a set bit, as
/// `(offset, clump)`.
pub fn for_each_set_clump8(bits: &[u64], size: usize) -> Clump8Iter<'_> {
    let mut clump = 0;
    let start = find_first_clump8(&mut clump, bits, size);
    Clump8Iter {
        bits,
        size,
        start,
        clump,
    }
}

/// `find_random_bit()`: the position of a random set bit, or `size` if there
/// are none. A single set bit is found directly, with no draw.
pub fn find_random_bit(
    addr: &[u64],
    size: usize,
    rng: &mut impl crate::random::RandomSource,
) -> usize {
    let w = crate::bitmap::bitmap_weight(addr, size);
    match w {
        0 => size,
        1 => find_first_bit(addr, size),
        _ => find_nth_bit(addr, size, rng.get_random_u32_below(w as u32) as usize),
    }
}

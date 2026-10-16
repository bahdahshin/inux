// SPDX-License-Identifier: GPL-2.0
//! Parsing a bitmap from its decimal list format: `0,3-5`, `all`, `N`, and
//! the strided `range:used/group` form.
//!
//! Ported from `lib/bitmap-str.c`: `bitmap_parselist()` and the region
//! helpers under it. A string ends at a NUL, at a newline, or at the end of
//! the slice. Each region is parsed as in C, and the errors are the same:
//! `EINVAL` for a bad format, `ERANGE` for a bit past the mask, and
//! `EOVERFLOW` for a number wider than 32 bits.
//!
//! Also ported: the hex chunk parser `bitmap_parse()`.
//!
//! Not ported yet: the user-copy wrapper `bitmap_parselist_user()` and the
//! printers.

use crate::bitmap::{bitmap_clear, bitmap_set, bitmap_zero};
use crate::errno::{EINVAL, EOVERFLOW, ERANGE, Result};
use crate::hexdump::hex_to_bin;

/// `struct region`: one comma-separated part of the list. The parts are
/// inclusive, and `off` bits are set out of every `group_len` from `start`.
struct Region {
    start: u64,
    off: u64,
    group_len: u64,
    end: u64,
    nbits: u64,
}

/// The byte at `pos`, with the end of the slice read as NUL as in C.
fn byte(buf: &[u8], pos: usize) -> u8 {
    buf.get(pos).copied().unwrap_or(0)
}

/// `end_of_str()`: NUL or newline.
fn end_of_str(c: u8) -> bool {
    c == 0 || c == b'\n'
}

/// `__end_of_region()`: white space or a comma.
fn is_region_sep(c: u8) -> bool {
    matches!(c, b' ' | b'\t' | b'\n' | 0x0b | 0x0c | b'\r' | b',')
}

/// `end_of_region()`: the separator or the end of the string.
fn end_of_region(c: u8) -> bool {
    is_region_sep(c) || end_of_str(c)
}

/// `bitmap_find_region()`: skip leading separators. `None` at the end of
/// the string.
fn find_region(buf: &[u8], mut pos: usize) -> Option<usize> {
    while is_region_sep(byte(buf, pos)) {
        pos += 1;
    }
    if end_of_str(byte(buf, pos)) {
        None
    } else {
        Some(pos)
    }
}

/// `bitmap_getnum()`: a decimal number, or `N` for the last bit. Returns the
/// value and the position after it.
fn getnum(buf: &[u8], pos: usize, lastbit: u64) -> Result<(u64, usize)> {
    if byte(buf, pos) == b'N' {
        return Ok((lastbit, pos + 1));
    }

    let mut n: u64 = 0;
    let mut len = 0;
    while byte(buf, pos + len).is_ascii_digit() {
        let d = u64::from(byte(buf, pos + len) - b'0');
        n = n
            .checked_mul(10)
            .and_then(|n| n.checked_add(d))
            .ok_or(EOVERFLOW)?;
        len += 1;
    }
    if len == 0 {
        return Err(EINVAL);
    }
    if n > u64::from(u32::MAX) {
        return Err(EOVERFLOW);
    }
    Ok((n, pos + len))
}

/// `bitmap_parse_region()`: parse one part into `r`, returning the position
/// after it. A single number `n` is the range `n-n` with no pattern.
fn parse_region(buf: &[u8], mut pos: usize, r: &mut Region) -> Result<usize> {
    let lastbit = r.nbits.wrapping_sub(1);

    let is_all = buf
        .get(pos..pos + 3)
        .is_some_and(|s| s.eq_ignore_ascii_case(b"all"));
    if is_all {
        r.start = 0;
        r.end = lastbit;
        pos += 3;
    } else {
        let (start, p) = getnum(buf, pos, lastbit)?;
        r.start = start;
        pos = p;

        if end_of_region(byte(buf, pos)) {
            // No range end: a single bit.
            r.end = r.start;
            return Ok(no_pattern(r, pos));
        }
        if byte(buf, pos) != b'-' {
            return Err(EINVAL);
        }
        let (end, p) = getnum(buf, pos + 1, lastbit)?;
        r.end = end;
        pos = p;
    }

    // The optional `:used/group` pattern.
    if end_of_region(byte(buf, pos)) {
        return Ok(no_pattern(r, pos));
    }
    if byte(buf, pos) != b':' {
        return Err(EINVAL);
    }
    let (off, p) = getnum(buf, pos + 1, lastbit)?;
    r.off = off;
    pos = p;
    if byte(buf, pos) != b'/' {
        return Err(EINVAL);
    }
    let (group_len, p) = getnum(buf, pos + 1, lastbit)?;
    r.group_len = group_len;
    Ok(p)
}

/// The `no_pattern:` tail: the whole range is used, in one group.
fn no_pattern(r: &mut Region, pos: usize) -> usize {
    r.off = r.end + 1;
    r.group_len = r.end + 1;
    pos
}

/// `bitmap_check_region()`.
fn check_region(r: &Region) -> Result<()> {
    if r.start > r.end || r.group_len == 0 || r.off > r.group_len {
        return Err(EINVAL);
    }
    if r.end >= r.nbits {
        return Err(ERANGE);
    }
    Ok(())
}

/// `bitmap_set_region()`: set `off` bits from each group start in the range.
fn set_region(r: &Region, mask: &mut [u64]) {
    let mut start = r.start;
    while start <= r.end {
        let len = (r.end - start + 1).min(r.off);
        bitmap_set(mask, start as usize, len as usize);
        start += r.group_len;
    }
}

/// `bitmap_parselist()`: parse a comma-separated list of bits and ranges into
/// `mask`, which is cleared first and holds `nmaskbits` bits.
///
/// Returns `EINVAL` for a malformed region, `ERANGE` for a bit at or past
/// `nmaskbits`, and `EOVERFLOW` for a number wider than 32 bits.
pub fn bitmap_parselist(buf: &[u8], mask: &mut [u64], nmaskbits: usize) -> Result<()> {
    bitmap_zero(mask, nmaskbits);

    let mut pos = 0;
    while let Some(p) = find_region(buf, pos) {
        let mut r = Region {
            start: 0,
            off: 0,
            group_len: 0,
            end: 0,
            nbits: nmaskbits as u64,
        };
        pos = parse_region(buf, p, &mut r)?;
        check_region(&r)?;
        set_region(&r, mask);
    }
    Ok(())
}

/// The byte at a signed position, NUL outside the string.
fn at(buf: &[u8], pos: isize) -> u8 {
    if pos < 0 { 0 } else { byte(buf, pos as usize) }
}

/// `bitmap_find_region_reverse()`: skip separators backwards from `end`.
fn find_region_reverse(buf: &[u8], begin: isize, mut end: isize) -> isize {
    while begin <= end && is_region_sep(at(buf, end)) {
        end -= 1;
    }
    end
}

/// `bitmap_get_x32_reverse()`: read up to eight hex digits ending at `end`,
/// scanning backwards and stopping at a separator. Returns the value and the
/// position before the digits read. A ninth hex digit is `EOVERFLOW`.
fn get_x32_reverse(buf: &[u8], begin: isize, mut end: isize) -> Result<(u32, isize)> {
    let mut value: u32 = 0;
    let mut shift = 0;
    let mut done = false;
    while shift < 32 {
        let c = hex_to_bin(at(buf, end));
        end -= 1;
        if c < 0 {
            return Err(EINVAL);
        }
        value |= (c as u32) << shift;
        if begin > end || is_region_sep(at(buf, end)) {
            done = true;
            break;
        }
        shift += 4;
    }
    if !done {
        let overflow = hex_to_bin(at(buf, end)) >= 0;
        end -= 1;
        if overflow {
            return Err(EOVERFLOW);
        }
    }
    Ok((value, end))
}

/// Store the `chunk`th 32-bit word of the mask, which is little-endian.
fn set_chunk(mask: &mut [u64], chunk: usize, value: u32) {
    let word = chunk / 2;
    let shift = (chunk % 2) * 32;
    mask[word] &= !(0xffff_ffffu64 << shift);
    mask[word] |= u64::from(value) << shift;
}

/// `bitmap_parse()`: parse a hex string into `nmaskbits` bits. Each comma
/// separated group holds 32 bits, read from the right, so a short group
/// leaves its high bits zero. More than 32 bits in a group, or more groups
/// than the mask holds, is `EOVERFLOW`; a non-hex character is `EINVAL`.
///
/// `buf` is read up to `buflen` bytes or the first newline or NUL, whichever
/// comes first. Pass `usize::MAX` to read to the end of the slice.
pub fn bitmap_parse(buf: &[u8], buflen: usize, mask: &mut [u64], nmaskbits: usize) -> Result<()> {
    let s = &buf[..buflen.min(buf.len())];
    let stop = s
        .iter()
        .position(|&c| c == b'\n' || c == 0)
        .unwrap_or(s.len());

    let chunks = nmaskbits.div_ceil(32);
    let mut remaining = chunks;
    let mut chunk = 0;
    let begin: isize = 0;
    let mut end = stop as isize - 1;

    loop {
        end = find_region_reverse(s, begin, end);
        if begin > end {
            break;
        }
        if remaining == 0 {
            return Err(EOVERFLOW);
        }
        remaining -= 1;
        let (value, next) = get_x32_reverse(s, begin, end)?;
        end = next;
        set_chunk(mask, chunk, value);
        chunk += 1;
    }

    // Groups not written are cleared from the parsed width up to `nmaskbits`.
    // A set bit between `nmaskbits` and the parsed width is an overflow.
    let unset = (chunks - remaining) * 32;
    if unset < nmaskbits {
        bitmap_clear(mask, unset, nmaskbits - unset);
        return Ok(());
    }
    for bit in nmaskbits..unset {
        if (mask[bit / 64] >> (bit % 64)) & 1 == 1 {
            return Err(EOVERFLOW);
        }
    }
    Ok(())
}

// SPDX-License-Identifier: GPL-2.0
//! UCS-2 (16-bit code unit) string handling: length, sized copy, compare
//! and conversion to UTF-8.
//!
//! Ported from `lib/ucs2_string.c`. Linux's UCS-2 strings are arrays of
//! `ucs2_char_t` (`u16`) that end at a NUL unit. Here they are slices: a
//! string ends at its first zero unit or at the end of the slice, as in
//! [`string`](crate::string). The `EXPORT_SYMBOL` and module boilerplate
//! are dropped.
//!
//! Lengths and counts are in code units unless a function name or doc says
//! "bytes". Conversion writes into a caller-supplied byte buffer.

use crate::errno::{E2BIG, Result};

/// `ucs2_char_t`: one UCS-2 code unit.
pub type Ucs2Char = u16;

/// `ucs2_strnlen()`: the number of units before the first NUL, looking at
/// most `maxlength` units. The NUL is not counted.
pub fn ucs2_strnlen(s: &[Ucs2Char], maxlength: usize) -> usize {
    s.iter().take(maxlength).take_while(|&&c| c != 0).count()
}

/// `ucs2_strlen()`: the number of units before the first NUL in `s`.
pub fn ucs2_strlen(s: &[Ucs2Char]) -> usize {
    ucs2_strnlen(s, s.len())
}

/// `ucs2_strsize()`: the size in bytes of the string in `data`, counting at
/// most `maxlength` bytes. This is not the number of characters.
pub fn ucs2_strsize(data: &[Ucs2Char], maxlength: usize) -> usize {
    ucs2_strnlen(data, maxlength / size_of::<Ucs2Char>()) * size_of::<Ucs2Char>()
}

/// `ucs2_strscpy()`: copy `src` into `dst` as far as it fits, and always
/// NUL-terminate `dst` unless it is empty.
///
/// Returns the number of units copied, not counting the NUL. Returns
/// `E2BIG` if `dst` is empty or `src` was cut short; `dst` is still
/// terminated in the cut case. A `src` with no NUL is read as if it ended
/// there.
pub fn ucs2_strscpy(dst: &mut [Ucs2Char], src: &[Ucs2Char]) -> Result<usize> {
    let count = dst.len();
    if count == 0 {
        return Err(E2BIG);
    }

    for (res, slot) in dst.iter_mut().enumerate() {
        let c = src.get(res).copied().unwrap_or(0);
        *slot = c;
        if c == 0 {
            return Ok(res);
        }
    }

    dst[count - 1] = 0;
    Err(E2BIG)
}

/// `ucs2_strncmp()`: compare at most `len` units. Returns a negative,
/// zero or positive value, like `strncmp()`. Equal strings that end at a
/// NUL compare equal even if `len` is larger.
pub fn ucs2_strncmp(a: &[Ucs2Char], b: &[Ucs2Char], len: usize) -> i32 {
    for i in 0..len {
        let x = a.get(i).copied().unwrap_or(0);
        let y = b.get(i).copied().unwrap_or(0);
        if x < y {
            return -1;
        }
        if x > y {
            return 1;
        }
        if x == 0 {
            return 0;
        }
    }
    0
}

/// `ucs2_utf8size()`: the number of bytes the UTF-8 form of `src` takes,
/// not counting a terminator. Units below 0x80 take one byte, below 0x800
/// two, and the rest three.
pub fn ucs2_utf8size(src: &[Ucs2Char]) -> usize {
    src.iter()
        .take_while(|&&c| c != 0)
        .map(|&c| match c {
            0x800.. => 3,
            0x80.. => 2,
            _ => 1,
        })
        .sum()
}

/// `ucs2_as_utf8()`: convert `src` to UTF-8 in `dest`, writing at most
/// `maxlength` bytes and only whole characters. A NUL is written after the
/// last character when there is room for it.
///
/// Returns the number of bytes written, not counting that NUL. `dest` must
/// hold at least `maxlength` bytes.
pub fn ucs2_as_utf8(dest: &mut [u8], src: &[Ucs2Char], maxlength: usize) -> usize {
    let limit = ucs2_strnlen(src, maxlength);
    let mut budget = maxlength;
    let mut j = 0;

    for &c in src.iter().take(limit) {
        if budget == 0 {
            break;
        }
        if c >= 0x800 {
            if budget < 3 {
                break;
            }
            budget -= 3;
            dest[j] = 0xe0 | ((c & 0xf000) >> 12) as u8;
            dest[j + 1] = 0x80 | ((c & 0x0fc0) >> 6) as u8;
            dest[j + 2] = 0x80 | (c & 0x003f) as u8;
            j += 3;
        } else if c >= 0x80 {
            if budget < 2 {
                break;
            }
            budget -= 2;
            dest[j] = 0xc0 | ((c & 0x7c0) >> 6) as u8;
            dest[j + 1] = 0x80 | (c & 0x03f) as u8;
            j += 2;
        } else {
            budget -= 1;
            dest[j] = (c & 0x7f) as u8;
            j += 1;
        }
    }

    if budget > 0 {
        dest[j] = 0;
    }
    j
}

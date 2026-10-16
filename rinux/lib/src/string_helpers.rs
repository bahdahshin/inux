// SPDX-License-Identifier: GPL-2.0
//! Human-readable sizes, string unescaping and string escaping.
//!
//! Ported from `lib/string_helpers.c`: `string_get_size()`,
//! `string_unescape()` and `string_escape_mem()`. The C functions write into
//! a `char *` with a size; here the destination is a byte slice, and the
//! return value is the length the C function returns, which can exceed the
//! slice when the output is truncated.
//!
//! Not ported yet: `parse_int_array()`, the `kstrdup_quotable*` family,
//! `kasprintf_strarray()` and the other allocating helpers, the
//! `match_string()` family, and `memcpy_and_pad()`'s fortify hooks.

use crate::errno::{EINVAL, ENOENT, Result};
use crate::hexdump::hex_to_bin;

/// `STRING_UNITS_10`: powers of 1000 (SI).
pub const STRING_UNITS_10: u32 = 0;
/// `STRING_UNITS_2`: powers of 1024 (binary).
pub const STRING_UNITS_2: u32 = 1;
/// `STRING_UNITS_MASK`: the base bit of a `units` argument.
pub const STRING_UNITS_MASK: u32 = 1;
/// `STRING_UNITS_NO_SPACE`: no space before the unit.
pub const STRING_UNITS_NO_SPACE: u32 = 1 << 30;
/// `STRING_UNITS_NO_BYTES`: no trailing `B`.
pub const STRING_UNITS_NO_BYTES: u32 = 1 << 31;

const UNITS_10: [&str; 9] = ["", "k", "M", "G", "T", "P", "E", "Z", "Y"];
const UNITS_2: [&str; 9] = ["", "Ki", "Mi", "Gi", "Ti", "Pi", "Ei", "Zi", "Yi"];

/// A fixed buffer the formatted text is assembled in, so the functions here
/// need no allocator.
struct Sink {
    data: [u8; 80],
    len: usize,
}

impl Sink {
    fn new() -> Sink {
        Sink {
            data: [0; 80],
            len: 0,
        }
    }

    fn push(&mut self, s: &[u8]) {
        for &b in s {
            if self.len < self.data.len() {
                self.data[self.len] = b;
            }
            self.len += 1;
        }
    }

    fn push_u32(&mut self, mut n: u32) {
        let mut digits = [0u8; 10];
        let mut len = 0;
        loop {
            digits[len] = b'0' + (n % 10) as u8;
            len += 1;
            n /= 10;
            if n == 0 {
                break;
            }
        }
        for &d in digits[..len].iter().rev() {
            self.push(&[d]);
        }
    }
}

/// The `snprintf()` contract: copy at most `len - 1` bytes and NUL-terminate
/// when `buf` has room, and return the length the text would have had.
fn snprintf_out(buf: &mut [u8], text: &[u8]) -> usize {
    if !buf.is_empty() {
        let k = text.len().min(buf.len() - 1);
        buf[..k].copy_from_slice(&text[..k]);
        buf[k] = 0;
    }
    text.len()
}

/// `string_get_size()`: format `size` blocks of `blk_size` bytes as a value
/// with a unit, such as `1.50 kB` or `1.00 KiB`, into `buf`. The value is
/// reduced with Napier's algorithm so the multiplication cannot overflow,
/// then rounded to three significant digits.
///
/// Returns the length of the full text, as `snprintf()` does.
pub fn string_get_size(mut size: u64, mut blk_size: u64, units: u32, buf: &mut [u8]) -> usize {
    let units_base = units & STRING_UNITS_MASK;
    let divisor: u64 = if units_base == STRING_UNITS_2 {
        1024
    } else {
        1000
    };
    let rounding = [500u32, 50, 5];

    let mut i = 0usize;
    let mut remainder: u32 = 0;
    let mut tmp = Sink::new();

    if blk_size == 0 {
        size = 0;
    }

    if size != 0 {
        // Reduce both coefficients to just under 32 bits.
        while blk_size >> 32 != 0 {
            blk_size /= divisor;
            i += 1;
        }
        while size >> 32 != 0 {
            size /= divisor;
            i += 1;
        }

        size *= blk_size;

        // Reduce the product until it is under the divisor.
        while size >= divisor {
            remainder = (size % divisor) as u32;
            size /= divisor;
            i += 1;
        }

        // The digits of precision the remainder needs.
        let mut sf_cap = size as u32;
        let mut j = 0usize;
        while sf_cap * 10 < 1000 {
            sf_cap *= 10;
            j += 1;
        }

        if units_base == STRING_UNITS_2 {
            remainder *= 1000;
            remainder >>= 10;
        }

        // Round up at the digit below the printed precision.
        remainder += rounding[j];
        if remainder >= 1000 {
            remainder -= 1000;
            size += 1;
        }

        if j > 0 {
            // `.%03u` cut to `j + 1` bytes: the point and `j` digits.
            let mut frac = Sink::new();
            frac.push(b".");
            let r = remainder;
            frac.push(&[
                b'0' + (r / 100) as u8,
                b'0' + (r / 10 % 10) as u8,
                b'0' + (r % 10) as u8,
            ]);
            tmp.push(&frac.data[..j + 1]);
        }
    }

    let unit: &str = if i >= UNITS_2.len() {
        "UNK"
    } else if units_base == STRING_UNITS_2 {
        UNITS_2[i]
    } else {
        UNITS_10[i]
    };

    let mut out = Sink::new();
    out.push_u32(size as u32);
    out.push(&tmp.data[..tmp.len.min(tmp.data.len())]);
    if units & STRING_UNITS_NO_SPACE == 0 {
        out.push(b" ");
    }
    out.push(unit.as_bytes());
    if units & STRING_UNITS_NO_BYTES == 0 {
        out.push(b"B");
    }
    snprintf_out(buf, &out.data[..out.len.min(out.data.len())])
}

/// `UNESCAPE_SPACE`: `\n \r \t \v \f`.
pub const UNESCAPE_SPACE: u32 = 1 << 0;
/// `UNESCAPE_OCTAL`: `\ooo`.
pub const UNESCAPE_OCTAL: u32 = 1 << 1;
/// `UNESCAPE_HEX`: `\xhh`.
pub const UNESCAPE_HEX: u32 = 1 << 2;
/// `UNESCAPE_SPECIAL`: `\" \\ \a \e`.
pub const UNESCAPE_SPECIAL: u32 = 1 << 3;
/// Every unescape class.
pub const UNESCAPE_ANY: u32 = UNESCAPE_SPACE | UNESCAPE_OCTAL | UNESCAPE_HEX | UNESCAPE_SPECIAL;

/// The byte at `pos`, NUL past the end, as the C string reads.
fn byte(s: &[u8], pos: usize) -> u8 {
    s.get(pos).copied().unwrap_or(0)
}

/// Write `c` at `*out` if it is in bounds, and advance `*out` regardless.
fn put(dst: &mut [u8], out: &mut usize, c: u8) {
    if *out < dst.len() {
        dst[*out] = c;
    }
    *out += 1;
}

fn isodigit(c: u8) -> bool {
    (b'0'..=b'7').contains(&c)
}

fn unescape_space(src: &[u8], s: &mut usize, dst: &mut [u8], out: &mut usize) -> bool {
    let c = match byte(src, *s) {
        b'n' => b'\n',
        b'r' => b'\r',
        b't' => b'\t',
        b'v' => 0x0b,
        b'f' => 0x0c,
        _ => return false,
    };
    put(dst, out, c);
    *s += 1;
    true
}

fn unescape_octal(src: &[u8], s: &mut usize, dst: &mut [u8], out: &mut usize) -> bool {
    if !isodigit(byte(src, *s)) {
        return false;
    }
    let start = *s;
    let mut q = *s;
    let mut num = (byte(src, q)) & 7;
    q += 1;
    while num < 32 && isodigit(byte(src, q)) && q - start < 3 {
        num <<= 3;
        num += byte(src, q) & 7;
        q += 1;
    }
    put(dst, out, num);
    *s = q;
    true
}

fn unescape_hex(src: &[u8], s: &mut usize, dst: &mut [u8], out: &mut usize) -> bool {
    if byte(src, *s) != b'x' {
        return false;
    }
    let mut q = *s + 1;
    let digit = hex_to_bin(byte(src, q));
    q += 1;
    if digit < 0 {
        return false;
    }
    let mut num = digit as u8;
    let next = hex_to_bin(byte(src, q));
    if next >= 0 {
        q += 1;
        num = (num << 4) | next as u8;
    }
    put(dst, out, num);
    *s = q;
    true
}

fn unescape_special(src: &[u8], s: &mut usize, dst: &mut [u8], out: &mut usize) -> bool {
    let c = match byte(src, *s) {
        b'"' => b'"',
        b'\\' => b'\\',
        b'a' => 0x07,
        b'e' => 0x1b,
        _ => return false,
    };
    put(dst, out, c);
    *s += 1;
    true
}

/// `string_unescape()`: decode the backslash escapes selected by `flags`
/// from `src` into `dst`, NUL-terminating `dst` when there is room. An
/// escape that is not enabled, or does not parse, is kept as written.
///
/// `dst` bounds the output: at most `dst.len() - 1` bytes are produced, as
/// the C `size` argument does. The C function treats a size of 0 as no
/// limit; a slice cannot be unbounded, so an empty `dst` produces nothing.
///
/// Returns the length of the unescaped string, not counting the NUL.
pub fn string_unescape(src: &[u8], dst: &mut [u8], flags: u32) -> usize {
    let mut size = dst.len();
    let mut s = 0usize;
    let mut out = 0usize;

    // `while (*src && --size)`
    loop {
        if byte(src, s) == 0 || size <= 1 {
            break;
        }
        size -= 1;

        if byte(src, s) == b'\\' && byte(src, s + 1) != 0 && size > 1 {
            s += 1;
            size -= 1;

            if flags & UNESCAPE_SPACE != 0 && unescape_space(src, &mut s, dst, &mut out) {
                continue;
            }
            if flags & UNESCAPE_OCTAL != 0 && unescape_octal(src, &mut s, dst, &mut out) {
                continue;
            }
            if flags & UNESCAPE_HEX != 0 && unescape_hex(src, &mut s, dst, &mut out) {
                continue;
            }
            if flags & UNESCAPE_SPECIAL != 0 && unescape_special(src, &mut s, dst, &mut out) {
                continue;
            }
            put(dst, &mut out, b'\\');
        }
        put(dst, &mut out, byte(src, s));
        s += 1;
    }
    put(dst, &mut out, 0);
    // The NUL is not part of the length.
    out - 1
}

/// `ESCAPE_SPACE`: `\n \r \t \v \f`.
pub const ESCAPE_SPACE: u32 = 1 << 0;
/// `ESCAPE_SPECIAL`: `\\ \a \e \"`.
pub const ESCAPE_SPECIAL: u32 = 1 << 1;
/// `ESCAPE_NULL`: `\0`.
pub const ESCAPE_NULL: u32 = 1 << 2;
/// `ESCAPE_OCTAL`: `\ooo`.
pub const ESCAPE_OCTAL: u32 = 1 << 3;
/// Every symbol class.
pub const ESCAPE_ANY: u32 = ESCAPE_SPACE | ESCAPE_OCTAL | ESCAPE_SPECIAL | ESCAPE_NULL;
/// `ESCAPE_NP`: pass printable characters through.
pub const ESCAPE_NP: u32 = 1 << 4;
/// `ESCAPE_ANY` with `ESCAPE_NP`.
pub const ESCAPE_ANY_NP: u32 = ESCAPE_ANY | ESCAPE_NP;
/// `ESCAPE_HEX`: `\xhh`, after the other classes.
pub const ESCAPE_HEX: u32 = 1 << 5;
/// `ESCAPE_NA`: pass ASCII through.
pub const ESCAPE_NA: u32 = 1 << 6;
/// `ESCAPE_NAP`: pass printable ASCII through.
pub const ESCAPE_NAP: u32 = 1 << 7;
/// `ESCAPE_APPEND`: the `only` set is excluded from the pass-through rules.
pub const ESCAPE_APPEND: u32 = 1 << 8;

/// The output of an escape, with the `end` bound of the C code: bytes past
/// the buffer are counted but not stored.
struct Out<'a> {
    buf: &'a mut [u8],
    pos: usize,
}

impl Out<'_> {
    fn put(&mut self, c: u8) {
        if self.pos < self.buf.len() {
            self.buf[self.pos] = c;
        }
        self.pos += 1;
    }
}

const HEX_ASC: &[u8; 16] = b"0123456789abcdef";

fn isprint(c: u8) -> bool {
    (0x20..=0x7e).contains(&c)
}

fn escape_passthrough(c: u8, out: &mut Out) -> bool {
    out.put(c);
    true
}

fn escape_space(c: u8, out: &mut Out) -> bool {
    let to = match c {
        b'\n' => b'n',
        b'\r' => b'r',
        b'\t' => b't',
        0x0b => b'v',
        0x0c => b'f',
        _ => return false,
    };
    out.put(b'\\');
    out.put(to);
    true
}

fn escape_special(c: u8, out: &mut Out) -> bool {
    let to = match c {
        b'\\' => b'\\',
        0x07 => b'a',
        0x1b => b'e',
        b'"' => b'"',
        _ => return false,
    };
    out.put(b'\\');
    out.put(to);
    true
}

fn escape_null(c: u8, out: &mut Out) -> bool {
    if c != 0 {
        return false;
    }
    out.put(b'\\');
    out.put(b'0');
    true
}

fn escape_octal(c: u8, out: &mut Out) -> bool {
    out.put(b'\\');
    out.put(((c >> 6) & 0x07) + b'0');
    out.put(((c >> 3) & 0x07) + b'0');
    out.put((c & 0x07) + b'0');
    true
}

fn escape_hex(c: u8, out: &mut Out) -> bool {
    out.put(b'\\');
    out.put(b'x');
    out.put(HEX_ASC[(c >> 4) as usize]);
    out.put(HEX_ASC[(c & 0x0f) as usize]);
    true
}

/// `string_escape_mem()`: escape `src` into `dst` according to `flags`.
///
/// Each byte is tried against the rules in the C order: the `only` set, the
/// printable and ASCII pass-throughs, then the symbol classes. A byte no
/// rule takes is copied as is. `only`, if not empty, is a NUL-terminated set
/// of bytes that the pass-through rules treat specially.
///
/// Returns the length of the escaped output, which can exceed `dst.len()`
/// when `dst` is too small; only the bytes that fit are stored.
pub fn string_escape_mem(src: &[u8], dst: &mut [u8], flags: u32, only: &[u8]) -> usize {
    let is_dict = byte(only, 0) != 0;
    let only_set = only.iter().take_while(|&&b| b != 0);
    let mut out = Out { buf: dst, pos: 0 };
    let is_append = flags & ESCAPE_APPEND != 0;

    for &c in src {
        // `strchr(only, c)` finds the NUL terminator for a zero byte.
        let in_dict = is_dict && (c == 0 || only_set.clone().any(|&b| b == c));

        if !(is_append || in_dict) && is_dict && escape_passthrough(c, &mut out) {
            continue;
        }
        if !(is_append && in_dict)
            && c < 128
            && isprint(c)
            && flags & ESCAPE_NAP != 0
            && escape_passthrough(c, &mut out)
        {
            continue;
        }
        if !(is_append && in_dict)
            && isprint(c)
            && flags & ESCAPE_NP != 0
            && escape_passthrough(c, &mut out)
        {
            continue;
        }
        if !(is_append && in_dict)
            && c < 128
            && flags & ESCAPE_NA != 0
            && escape_passthrough(c, &mut out)
        {
            continue;
        }
        if flags & ESCAPE_SPACE != 0 && escape_space(c, &mut out) {
            continue;
        }
        if flags & ESCAPE_SPECIAL != 0 && escape_special(c, &mut out) {
            continue;
        }
        if flags & ESCAPE_NULL != 0 && escape_null(c, &mut out) {
            continue;
        }
        // Octal and hex always go last.
        if flags & ESCAPE_OCTAL != 0 && escape_octal(c, &mut out) {
            continue;
        }
        if flags & ESCAPE_HEX != 0 && escape_hex(c, &mut out) {
            continue;
        }
        escape_passthrough(c, &mut out);
    }

    out.pos
}

/// `strim()`: the view of `s` without leading and trailing white space,
/// as a C string: the slice ends at its first NUL. The C version writes the
/// terminator in place; this returns the trimmed view instead, which leaves
/// `s` unchanged.
pub fn strim(s: &[u8]) -> &[u8] {
    let len = s.iter().position(|&c| c == 0).unwrap_or(s.len());
    let s = &s[..len];
    let start = s.iter().position(|&c| !is_space(c)).unwrap_or(s.len());
    let end = s
        .iter()
        .rposition(|&c| !is_space(c))
        .map_or(start, |e| e + 1);
    &s[start..end.max(start)]
}

/// `isspace()`: space, tab, newline, vertical tab, form feed, carriage return.
fn is_space(c: u8) -> bool {
    matches!(c, b' ' | b'\t' | b'\n' | 0x0b | 0x0c | b'\r')
}

/// `sysfs_streq()`: equal as sysfs writes compare, where a trailing newline
/// on either side is ignored.
pub fn sysfs_streq(s1: &[u8], s2: &[u8]) -> bool {
    let mut i = 0;
    while byte(s1, i) != 0 && byte(s1, i) == byte(s2, i) {
        i += 1;
    }
    let (a, b) = (byte(s1, i), byte(s2, i));
    if a == b {
        return true;
    }
    if a == 0 && b == b'\n' && byte(s2, i + 1) == 0 {
        return true;
    }
    a == b'\n' && byte(s1, i + 1) == 0 && b == 0
}

/// `match_string()`: the index of `string` in `array`, which ends at its
/// first `None` entry, or `EINVAL` when it is absent. Only the first `n`
/// entries are searched.
pub fn match_string(array: &[Option<&[u8]>], n: usize, string: &[u8]) -> Result<usize> {
    for (index, item) in array.iter().take(n).enumerate() {
        match item {
            None => break,
            Some(item) if strcmp_eq(item, string) => return Ok(index),
            Some(_) => {}
        }
    }
    Err(EINVAL)
}

/// `__sysfs_match_string()`: [`match_string`] with [`sysfs_streq`] as the
/// comparison.
pub fn sysfs_match_string(array: &[Option<&[u8]>], n: usize, string: &[u8]) -> Result<usize> {
    for (index, item) in array.iter().take(n).enumerate() {
        match item {
            None => break,
            Some(item) if sysfs_streq(item, string) => return Ok(index),
            Some(_) => {}
        }
    }
    Err(EINVAL)
}

/// `strcmp() == 0` on two C strings.
fn strcmp_eq(a: &[u8], b: &[u8]) -> bool {
    let a = &a[..a.iter().position(|&c| c == 0).unwrap_or(a.len())];
    let b = &b[..b.iter().position(|&c| c == 0).unwrap_or(b.len())];
    a == b
}

/// `strreplace()`: replace every `old` byte with `new` up to the first NUL.
pub fn strreplace(s: &mut [u8], old: u8, new: u8) {
    for c in s.iter_mut() {
        if *c == 0 {
            break;
        }
        if *c == old {
            *c = new;
        }
    }
}

/// `parse_int_array()`: parses a comma-separated list of integers with
/// [`get_options`]. Element 0 of the result is the count, and the integers
/// follow, as C's array does. Returns `ENOENT` when the string has none.
///
/// Behind the `alloc` feature, since the result is a `Vec`.
#[cfg(feature = "alloc")]
pub fn parse_int_array(buf: &[u8]) -> Result<alloc::vec::Vec<i32>> {
    use crate::cmdline::get_options;

    // Validation pass: with nints of 0, element 0 gets the count.
    let mut count = [0i32; 1];
    get_options(buf, 0, &mut count);
    let nints = count[0];
    if nints == 0 {
        return Err(ENOENT);
    }

    let mut ints = alloc::vec![0i32; nints as usize + 1];
    get_options(buf, nints + 1, &mut ints);
    Ok(ints)
}

#[cfg(all(test, feature = "alloc"))]
mod parse_int_array_tests {
    use super::*;

    #[test]
    fn comma_separated_integers_are_counted_and_listed() {
        assert_eq!(parse_int_array(b"1,2,3").unwrap(), alloc::vec![3, 1, 2, 3]);
        assert_eq!(parse_int_array(b"-4").unwrap(), alloc::vec![1, -4]);
    }

    #[test]
    fn a_string_with_no_integers_is_enoent() {
        assert_eq!(parse_int_array(b""), Err(ENOENT));
        assert_eq!(parse_int_array(b"abc"), Err(ENOENT));
    }
}

/// `kstrdup_quotable()`: a copy of the C string `src` with the characters of
/// C's set (form feed, newline, carriage return, tab, vertical tab, bell,
/// escape, backslash and double quote) hex-escaped as `\xhh`. Other bytes are
/// copied. `None` for a missing source, which is `NULL` in C.
///
/// Returns the escaped bytes without the trailing NUL, the same convention
/// as `kasprintf()`. Behind the `alloc` feature.
#[cfg(feature = "alloc")]
pub fn kstrdup_quotable(src: Option<&[u8]>) -> Option<alloc::vec::Vec<u8>> {
    // The C string ends at its first NUL.
    let src = src?;
    let src = &src[..src.iter().position(|&b| b == 0).unwrap_or(src.len())];
    let flags = ESCAPE_HEX;
    // C's escape set, with a NUL to end it.
    let esc: &[u8] = b"\x0c\n\r\t\x0b\x07\x1b\\\"\0";

    let dlen = string_escape_mem(src, &mut [], flags, esc);
    let mut dst = alloc::vec![0u8; dlen];
    let written = string_escape_mem(src, &mut dst, flags, esc);
    debug_assert_eq!(written, dlen);
    Some(dst)
}

#[cfg(all(test, feature = "alloc"))]
mod quotable_tests {
    use super::*;

    #[test]
    fn plain_text_is_unchanged() {
        assert_eq!(kstrdup_quotable(Some(b"hello")).unwrap(), b"hello");
    }

    #[test]
    fn listed_controls_and_quotes_are_hex_escaped() {
        // With ESCAPE_HEX the listed characters are hex-escaped, not given the
        // named forms: \n is \x0a, \t is \x09, " is \x22, \ is \x5c.
        assert_eq!(
            kstrdup_quotable(Some(b"a\nb\t\"c\\")).unwrap(),
            br"a\x0ab\x09\x22c\x5c"
        );
    }

    #[test]
    fn bytes_outside_the_listed_set_pass_through() {
        // \x01 is not in the set, so it is copied as it is.
        assert_eq!(kstrdup_quotable(Some(b"\x01")).unwrap(), b"\x01");
    }

    #[test]
    fn the_string_ends_at_its_first_nul() {
        assert_eq!(kstrdup_quotable(Some(b"ab\0cd")).unwrap(), b"ab");
    }

    #[test]
    fn a_missing_source_is_none() {
        assert!(kstrdup_quotable(None).is_none());
    }
}

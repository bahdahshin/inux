// SPDX-License-Identifier: GPL-2.0
//! String and memory routines.
//!
//! Ported from `lib/string.c`, the inline helpers in `include/linux/string.h`
//! and `memcpy_and_pad()` and `skip_spaces()` from `lib/string_helpers.c`.
//!
//! Copyright (C) 1991, 1992  Linus Torvalds
//!
//! # C strings as slices
//!
//! A C string here is a byte slice. The string ends at the first NUL byte,
//! or at the end of the slice if there is no NUL, so `b"abc"` and `b"abc\0"`
//! are the same string. Functions return indices where the C versions return
//! pointers; an index equal to the string length points at the terminator.
//!
//! Functions that write into a destination panic if the destination slice is
//! too small. In C those cases overflow the buffer.
//!
//! The word-at-a-time fast paths in the C file are performance details and
//! are not ported; every function returns what the C version returns.
//!
//! `memcpy()`, `memmove()` and `memset()` are not ported as library calls:
//! slices have `copy_from_slice()`, `copy_within()` and `fill()`, and the
//! compiler supplies the C-ABI symbols.

use core::cmp::min;

use crate::ctype::{isspace, tolower};
use crate::errno::{E2BIG, Result};

/// Byte `i` of a C string, or NUL past the end of the slice.
#[inline]
fn at(s: &[u8], i: usize) -> u8 {
    s.get(i).copied().unwrap_or(0)
}

/// `strlen()`: length of the string, not counting the terminator.
pub fn strlen(s: &[u8]) -> usize {
    s.iter().position(|&c| c == 0).unwrap_or(s.len())
}

/// `strnlen()`: like [`strlen`], but looks at no more than `count` bytes.
pub fn strnlen(s: &[u8], count: usize) -> usize {
    strlen(&s[..min(count, s.len())])
}

/// `strcmp()`: returns -1, 0 or 1. Bytes compare as unsigned.
pub fn strcmp(cs: &[u8], ct: &[u8]) -> i32 {
    let mut i = 0;
    loop {
        let (c1, c2) = (at(cs, i), at(ct, i));
        if c1 != c2 {
            return if c1 < c2 { -1 } else { 1 };
        }
        if c1 == 0 {
            return 0;
        }
        i += 1;
    }
}

/// `strncmp()`: [`strcmp`] limited to the first `count` bytes.
pub fn strncmp(cs: &[u8], ct: &[u8], count: usize) -> i32 {
    for i in 0..count {
        let (c1, c2) = (at(cs, i), at(ct, i));
        if c1 != c2 {
            return if c1 < c2 { -1 } else { 1 };
        }
        if c1 == 0 {
            break;
        }
    }
    0
}

/// `strcasecmp()`: case-insensitive compare. Returns the difference of the
/// first lower-cased bytes that differ.
pub fn strcasecmp(s1: &[u8], s2: &[u8]) -> i32 {
    let mut i = 0;
    loop {
        let c1 = tolower(at(s1, i)) as i32;
        let c2 = tolower(at(s2, i)) as i32;
        if c1 != c2 || c1 == 0 {
            return c1 - c2;
        }
        i += 1;
    }
}

/// `strncasecmp()`: [`strcasecmp`] limited to the first `len` bytes.
pub fn strncasecmp(s1: &[u8], s2: &[u8], len: usize) -> i32 {
    if len == 0 {
        return 0;
    }
    let (mut c1, mut c2);
    let mut i = 0;
    loop {
        c1 = at(s1, i);
        c2 = at(s2, i);
        i += 1;
        if c1 == 0 || c2 == 0 {
            break;
        }
        if c1 != c2 {
            c1 = tolower(c1);
            c2 = tolower(c2);
            if c1 != c2 {
                break;
            }
        }
        if i == len {
            break;
        }
    }
    c1 as i32 - c2 as i32
}

/// `strchr()`: index of the first `c`. Searching for NUL finds the terminator.
pub fn strchr(s: &[u8], c: u8) -> Option<usize> {
    let mut i = 0;
    loop {
        let ch = at(s, i);
        if ch == c {
            return Some(i);
        }
        if ch == 0 {
            return None;
        }
        i += 1;
    }
}

/// `strchrnul()`: index of the first `c`, or of the terminator.
pub fn strchrnul(s: &[u8], c: u8) -> usize {
    let mut i = 0;
    while at(s, i) != 0 && at(s, i) != c {
        i += 1;
    }
    i
}

/// `strnchrnul()`: like [`strchrnul`], looking at no more than `count` bytes.
pub fn strnchrnul(s: &[u8], count: usize, c: u8) -> usize {
    let mut i = 0;
    while i < count && at(s, i) != 0 && at(s, i) != c {
        i += 1;
    }
    i
}

/// `strrchr()`: index of the last `c`. Searching for NUL finds the terminator.
pub fn strrchr(s: &[u8], c: u8) -> Option<usize> {
    let mut last = None;
    let mut i = 0;
    loop {
        let ch = at(s, i);
        if ch == c {
            last = Some(i);
        }
        if ch == 0 {
            return last;
        }
        i += 1;
    }
}

/// `strnchr()`: [`strchr`] looking at no more than `count` bytes.
pub fn strnchr(s: &[u8], count: usize, c: u8) -> Option<usize> {
    for i in 0..count {
        let ch = at(s, i);
        if ch == c {
            return Some(i);
        }
        if ch == 0 {
            break;
        }
    }
    None
}

/// `strspn()`: length of the leading run of bytes that appear in `accept`.
pub fn strspn(s: &[u8], accept: &[u8]) -> usize {
    let mut i = 0;
    while at(s, i) != 0 && strchr(accept, at(s, i)).is_some() {
        i += 1;
    }
    i
}

/// `strcspn()`: length of the leading run of bytes that do not appear in `reject`.
pub fn strcspn(s: &[u8], reject: &[u8]) -> usize {
    let mut i = 0;
    while at(s, i) != 0 && strchr(reject, at(s, i)).is_none() {
        i += 1;
    }
    i
}

/// `strpbrk()`: index of the first byte of `cs` that appears in `ct`.
pub fn strpbrk(cs: &[u8], ct: &[u8]) -> Option<usize> {
    let mut i = 0;
    while at(cs, i) != 0 {
        if strchr(ct, at(cs, i)).is_some() {
            return Some(i);
        }
        i += 1;
    }
    None
}

/// `strsep()`: splits off the next token delimited by any byte in `ct`.
///
/// `*s` moves past the delimiter, or becomes `None` after the last token.
/// Empty tokens are returned too, like the libc function. The C version
/// writes a NUL over the delimiter; this one returns the token as a
/// sub-slice and leaves the input untouched.
pub fn strsep<'a>(s: &mut Option<&'a [u8]>, ct: &[u8]) -> Option<&'a [u8]> {
    let begin = (*s)?;
    match strpbrk(begin, ct) {
        Some(end) => {
            *s = Some(&begin[end + 1..]);
            Some(&begin[..end])
        }
        None => {
            *s = None;
            Some(&begin[..strlen(begin)])
        }
    }
}

/// `memset16()`: stores `v` in the first `count` elements of `s`.
pub fn memset16(s: &mut [u16], v: u16, count: usize) {
    s[..count].fill(v);
}

/// `memset32()`: stores `v` in the first `count` elements of `s`.
pub fn memset32(s: &mut [u32], v: u32, count: usize) {
    s[..count].fill(v);
}

/// `memset64()`: stores `v` in the first `count` elements of `s`.
pub fn memset64(s: &mut [u64], v: u64, count: usize) {
    s[..count].fill(v);
}

/// `memcmp()`: compares `count` bytes and returns the difference of the first
/// pair that differs, as unsigned bytes.
pub fn memcmp(cs: &[u8], ct: &[u8], count: usize) -> i32 {
    for (&a, &b) in cs[..count].iter().zip(&ct[..count]) {
        let res = a as i32 - b as i32;
        if res != 0 {
            return res;
        }
    }
    0
}

/// `bcmp()`: zero if and only if the first `len` bytes are equal.
pub fn bcmp(a: &[u8], b: &[u8], len: usize) -> i32 {
    memcmp(a, b, len)
}

/// `memscan()`: index of the first `c`, or `addr.len()` if there is none.
pub fn memscan(addr: &[u8], c: u8) -> usize {
    addr.iter().position(|&b| b == c).unwrap_or(addr.len())
}

/// `strstr()`: index of the first occurrence of `s2` in `s1`.
pub fn strstr(s1: &[u8], s2: &[u8]) -> Option<usize> {
    let l2 = strlen(s2);
    if l2 == 0 {
        return Some(0);
    }
    let l1 = strlen(s1);
    (0..l1.saturating_sub(l2 - 1)).find(|&i| s1[i..i + l2] == s2[..l2])
}

/// `strnstr()`: like [`strstr`], but only considers the first `len` bytes of `s1`.
///
/// As in C, this compares raw bytes and does not stop at a NUL in `s1`.
pub fn strnstr(s1: &[u8], s2: &[u8], len: usize) -> Option<usize> {
    let l2 = strlen(s2);
    if l2 == 0 {
        return Some(0);
    }
    let len = min(len, s1.len());
    (0..len.saturating_sub(l2 - 1)).find(|&i| s1[i..i + l2] == s2[..l2])
}

/// `memchr()`: index of the first `c` in `s`.
pub fn memchr(s: &[u8], c: u8) -> Option<usize> {
    s.iter().position(|&b| b == c)
}

/// `memchr_inv()`: index of the first byte that is not `c`.
pub fn memchr_inv(s: &[u8], c: u8) -> Option<usize> {
    s.iter().position(|&b| b != c)
}

/// `sized_strscpy()`: copies the C string `src` into `dest`, writing at most
/// `count` bytes including the terminator.
///
/// The result is always NUL-terminated unless `count` is 0. Returns the
/// number of bytes copied, not counting the terminator, or `E2BIG` if
/// `count` is 0 or `src` was truncated. `dest` must hold `count` bytes.
pub fn sized_strscpy(dest: &mut [u8], src: &[u8], count: usize) -> Result<usize> {
    if count == 0 || crate::warn_on_once!(count > i32::MAX as usize) {
        return Err(E2BIG);
    }
    let dest = &mut dest[..count];
    for (i, out) in dest[..count - 1].iter_mut().enumerate() {
        let c = at(src, i);
        *out = c;
        if c == 0 {
            return Ok(i);
        }
    }
    // Force NUL termination.
    dest[count - 1] = 0;
    if at(src, count - 1) != 0 {
        Err(E2BIG)
    } else {
        Ok(count - 1)
    }
}

/// `strscpy()`: [`sized_strscpy`] into the whole of `dest`.
pub fn strscpy(dest: &mut [u8], src: &[u8]) -> Result<usize> {
    let count = dest.len();
    sized_strscpy(dest, src, count)
}

/// `sized_strscpy_pad()`: [`sized_strscpy`], then zero the rest of the
/// first `count` bytes of `dest`.
pub fn sized_strscpy_pad(dest: &mut [u8], src: &[u8], count: usize) -> Result<usize> {
    let wrote = sized_strscpy(dest, src, count)?;
    if wrote < count {
        dest[wrote + 1..count].fill(0);
    }
    Ok(wrote)
}

/// `strscpy_pad()`: [`sized_strscpy_pad`] into the whole of `dest`.
pub fn strscpy_pad(dest: &mut [u8], src: &[u8]) -> Result<usize> {
    let count = dest.len();
    sized_strscpy_pad(dest, src, count)
}

/// `strcpy()`: copies `src` and its terminator to the start of `dest`.
pub fn strcpy(dest: &mut [u8], src: &[u8]) {
    stpcpy(dest, src);
}

/// `stpcpy()`: like [`strcpy`], but returns the index of the new terminator.
pub fn stpcpy(dest: &mut [u8], src: &[u8]) -> usize {
    let len = strlen(src);
    dest[..len].copy_from_slice(&src[..len]);
    dest[len] = 0;
    len
}

/// `strcat()`: appends `src` to the string in `dest`.
pub fn strcat(dest: &mut [u8], src: &[u8]) {
    let start = strlen(dest);
    stpcpy(&mut dest[start..], src);
}

/// `strncat()`: appends at most `count` bytes of `src`, then a terminator.
pub fn strncat(dest: &mut [u8], src: &[u8], count: usize) {
    if count == 0 {
        return;
    }
    let start = strlen(dest);
    let len = strnlen(src, count);
    dest[start..start + len].copy_from_slice(&src[..len]);
    dest[start + len] = 0;
}

/// `strlcat()`: appends `src` to `dest` so the result fits in `count` bytes
/// including the terminator. Returns the length the string would have
/// without truncation.
///
/// Panics if the string already in `dest` does not fit in `count` bytes.
pub fn strlcat(dest: &mut [u8], src: &[u8], count: usize) -> usize {
    let dsize = strlen(dest);
    let mut len = strlen(src);
    let res = dsize + len;

    crate::bug_on!(dsize >= count);

    let room = count - dsize;
    if len >= room {
        len = room - 1;
    }
    dest[dsize..dsize + len].copy_from_slice(&src[..len]);
    dest[dsize + len] = 0;
    res
}

/// `kbasename()`: the part of `path` after the last `/`.
pub fn kbasename(path: &[u8]) -> &[u8] {
    match strrchr(path, b'/') {
        Some(i) => &path[i + 1..],
        None => path,
    }
}

/// `str_has_prefix()`: `strlen(prefix)` if `s` starts with `prefix`, else 0.
pub fn str_has_prefix(s: &[u8], prefix: &[u8]) -> usize {
    let len = strlen(prefix);
    if strncmp(s, prefix, len) == 0 { len } else { 0 }
}

/// `strstarts()`: true if `s` starts with `prefix`.
pub fn strstarts(s: &[u8], prefix: &[u8]) -> bool {
    strncmp(s, prefix, strlen(prefix)) == 0
}

/// `strends()`: true if `s` ends with `suffix`.
pub fn strends(s: &[u8], suffix: &[u8]) -> bool {
    let str_len = strlen(s);
    let suffix_len = strlen(suffix);
    str_len >= suffix_len && strcmp(&s[str_len - suffix_len..], suffix) == 0
}

/// `memcpy_and_pad()`: copies `count` bytes of `src` into `dest` and fills
/// the rest of `dest` with `pad`. Copies only `dest.len()` bytes if `count`
/// is larger.
pub fn memcpy_and_pad(dest: &mut [u8], src: &[u8], count: usize, pad: u8) {
    if dest.len() > count {
        dest[..count].copy_from_slice(&src[..count]);
        dest[count..].fill(pad);
    } else {
        let n = dest.len();
        dest.copy_from_slice(&src[..n]);
    }
}

/// `skip_spaces()` from `lib/string_helpers.c`: index of the first byte of
/// `s` that is not white space.
pub fn skip_spaces(s: &[u8]) -> usize {
    let mut i = 0;
    while isspace(at(s, i)) {
        i += 1;
    }
    i
}

/// `strtomem()`: copies a C string into a byte array that is not
/// NUL-terminated, without padding.
pub fn strtomem(dest: &mut [u8], src: &[u8]) {
    let n = strnlen(src, min(src.len(), dest.len()));
    dest[..n].copy_from_slice(&src[..n]);
}

/// `strtomem_pad()`: like [`strtomem`], filling the rest of `dest` with `pad`.
pub fn strtomem_pad(dest: &mut [u8], src: &[u8], pad: u8) {
    let n = strnlen(src, min(src.len(), dest.len()));
    memcpy_and_pad(dest, src, n, pad);
}

/// `memtostr()`: copies a byte array that may lack a terminator into a
/// NUL-terminated string, truncating to fit.
pub fn memtostr(dest: &mut [u8], src: &[u8]) {
    assert!(!dest.is_empty() && !src.is_empty());
    let copy_len = min(dest.len() - 1, strnlen(src, src.len()));
    dest[..copy_len].copy_from_slice(&src[..copy_len]);
    dest[copy_len] = 0;
}

/// `memtostr_pad()`: like [`memtostr`], zero-filling the rest of `dest`.
pub fn memtostr_pad(dest: &mut [u8], src: &[u8]) {
    assert!(!dest.is_empty() && !src.is_empty());
    let copy_len = min(dest.len() - 1, strnlen(src, src.len()));
    dest[..copy_len].copy_from_slice(&src[..copy_len]);
    dest[copy_len..].fill(0);
}

// ---------------------------------------------------------------------------
// string_unescape(): decode C-style escape sequences. Ported from the
// unescape helpers of `lib/string_helpers.c`.
// ---------------------------------------------------------------------------

/// `UNESCAPE_SPACE`: decode `\f \n \r \t \v`.
pub const UNESCAPE_SPACE: u32 = 1 << 0;
/// `UNESCAPE_OCTAL`: decode `\NNN` octal escapes.
pub const UNESCAPE_OCTAL: u32 = 1 << 1;
/// `UNESCAPE_HEX`: decode `\xHH` hex escapes.
pub const UNESCAPE_HEX: u32 = 1 << 2;
/// `UNESCAPE_SPECIAL`: decode `\" \ \a \e`.
pub const UNESCAPE_SPECIAL: u32 = 1 << 3;
/// `UNESCAPE_ANY`: all of the above.
pub const UNESCAPE_ANY: u32 = UNESCAPE_SPACE | UNESCAPE_OCTAL | UNESCAPE_HEX | UNESCAPE_SPECIAL;

#[inline]
fn is_odigit(c: u8) -> bool {
    c.is_ascii_digit() && c <= b'7'
}

/// `unescape_space()`.
fn unescape_space(c: u8) -> Option<u8> {
    Some(match c {
        b'n' => b'\n',
        b'r' => b'\r',
        b't' => b'\t',
        b'v' => 0x0b,
        b'f' => 0x0c,
        _ => return None,
    })
}

/// `unescape_special()`.
fn unescape_special(c: u8) -> Option<u8> {
    Some(match c {
        b'"' => b'"',
        b'\\' => b'\\',
        b'a' => 0x07,
        b'e' => 0x1b,
        _ => return None,
    })
}

/// `unescape_octal()`: up to three octal digits. Returns the byte and the
/// number of source bytes consumed.
fn unescape_octal(rest: &[u8]) -> Option<(u8, usize)> {
    if rest.is_empty() || !is_odigit(rest[0]) {
        return None;
    }
    let mut num: u8 = rest[0] & 7;
    let mut n = 1;
    while num < 32 && n < 3 && n < rest.len() && is_odigit(rest[n]) {
        num = (num << 3) + (rest[n] & 7);
        n += 1;
    }
    Some((num, n))
}

/// `unescape_hex()`: `x` then one or two hex digits.
fn unescape_hex(rest: &[u8]) -> Option<(u8, usize)> {
    if rest.first() != Some(&b'x') || rest.len() < 2 {
        return None;
    }
    let d1 = crate::hexdump::hex_to_bin(rest[1]);
    if d1 < 0 {
        return None;
    }
    let mut num = d1 as u8;
    let mut n = 2;
    if rest.len() > 2 {
        let d2 = crate::hexdump::hex_to_bin(rest[2]);
        if d2 >= 0 {
            num = (num << 4) | d2 as u8;
            n = 3;
        }
    }
    Some((num, n))
}

/// `string_unescape()`: decode the escape sequences in `src` selected by
/// `flags`, writing the result to `dst`. Returns the number of bytes written.
/// Unlike the C version, `dst` is a byte slice and is not NUL-terminated.
pub fn string_unescape(src: &[u8], dst: &mut [u8], flags: u32) -> usize {
    let mut si = 0;
    let mut di = 0;
    while si < src.len() && di < dst.len() {
        if src[si] == b'\\' && si + 1 < src.len() {
            let rest = &src[si + 1..];
            if flags & UNESCAPE_SPACE != 0
                && let Some(b) = unescape_space(rest[0])
            {
                dst[di] = b;
                di += 1;
                si += 2;
                continue;
            }
            if flags & UNESCAPE_OCTAL != 0
                && let Some((b, n)) = unescape_octal(rest)
            {
                dst[di] = b;
                di += 1;
                si += 1 + n;
                continue;
            }
            if flags & UNESCAPE_HEX != 0
                && let Some((b, n)) = unescape_hex(rest)
            {
                dst[di] = b;
                di += 1;
                si += 1 + n;
                continue;
            }
            if flags & UNESCAPE_SPECIAL != 0
                && let Some(b) = unescape_special(rest[0])
            {
                dst[di] = b;
                di += 1;
                si += 2;
                continue;
            }
            // No handler matched: emit the backslash literally, then fall
            // through to copy the following byte.
            dst[di] = b'\\';
            di += 1;
            si += 1;
            if di >= dst.len() {
                break;
            }
        }
        dst[di] = src[si];
        di += 1;
        si += 1;
    }
    di
}

/// `string_unescape_any()`: decode every supported escape.
pub fn string_unescape_any(src: &[u8], dst: &mut [u8]) -> usize {
    string_unescape(src, dst, UNESCAPE_ANY)
}

// ---------------------------------------------------------------------------
// string_escape_mem(): the inverse -- encode bytes into a quotable form.
// Ported from the escape helpers of `lib/string_helpers.c`.
// ---------------------------------------------------------------------------

/// `ESCAPE_SPACE`: escape `\f \n \r \t \v`.
pub const ESCAPE_SPACE: u32 = 1 << 0;
/// `ESCAPE_SPECIAL`: escape `\\ \a \e "`.
pub const ESCAPE_SPECIAL: u32 = 1 << 1;
/// `ESCAPE_NULL`: escape a NUL byte as `\0`.
pub const ESCAPE_NULL: u32 = 1 << 2;
/// `ESCAPE_OCTAL`: escape as `\NNN`.
pub const ESCAPE_OCTAL: u32 = 1 << 3;
/// `ESCAPE_NP`: pass printable characters through unescaped.
pub const ESCAPE_NP: u32 = 1 << 4;
/// `ESCAPE_HEX`: escape as `\xHH`.
pub const ESCAPE_HEX: u32 = 1 << 5;
/// `ESCAPE_NA`: pass ASCII characters through unescaped.
pub const ESCAPE_NA: u32 = 1 << 6;
/// `ESCAPE_NAP`: pass printable ASCII characters through unescaped.
pub const ESCAPE_NAP: u32 = 1 << 7;
/// `ESCAPE_APPEND`: force escaping of the `only` characters on top of the
/// `ESCAPE_NP`/`ESCAPE_NA`/`ESCAPE_NAP` pass-through classes.
pub const ESCAPE_APPEND: u32 = 1 << 8;

/// `ESCAPE_ANY`: every escaping class.
pub const ESCAPE_ANY: u32 = ESCAPE_SPACE | ESCAPE_OCTAL | ESCAPE_SPECIAL | ESCAPE_NULL;
/// `ESCAPE_ANY_NP`: `ESCAPE_ANY` plus passing printables through.
pub const ESCAPE_ANY_NP: u32 = ESCAPE_ANY | ESCAPE_NP;

/// Writes `c` at `pos` if it is within `dst`, and returns `pos + 1`. Like the
/// C helpers, the running position advances even past the end so the return
/// value is the length that would be produced (as `snprintf` does).
#[inline]
fn emit(dst: &mut [u8], pos: usize, c: u8) -> usize {
    if pos < dst.len() {
        dst[pos] = c;
    }
    pos + 1
}

#[inline]
fn hex_asc(nibble: u8) -> u8 {
    b"0123456789abcdef"[(nibble & 0xf) as usize]
}

/// `escape_passthrough()`: copy `c` verbatim.
fn escape_passthrough(dst: &mut [u8], pos: usize, c: u8) -> usize {
    emit(dst, pos, c)
}

/// `escape_space()`: `\n \r \t \v \f`, or `None` for any other byte.
fn escape_space(dst: &mut [u8], pos: usize, c: u8) -> Option<usize> {
    let to = match c {
        b'\n' => b'n',
        b'\r' => b'r',
        b'\t' => b't',
        0x0b => b'v',
        0x0c => b'f',
        _ => return None,
    };
    let pos = emit(dst, pos, b'\\');
    Some(emit(dst, pos, to))
}

/// `escape_special()`: `\\ \a \e "`, or `None` for any other byte.
fn escape_special(dst: &mut [u8], pos: usize, c: u8) -> Option<usize> {
    let to = match c {
        b'\\' => b'\\',
        0x07 => b'a',
        0x1b => b'e',
        b'"' => b'"',
        _ => return None,
    };
    let pos = emit(dst, pos, b'\\');
    Some(emit(dst, pos, to))
}

/// `escape_null()`: a NUL byte as `\0`, or `None` otherwise.
fn escape_null(dst: &mut [u8], pos: usize, c: u8) -> Option<usize> {
    if c != 0 {
        return None;
    }
    let pos = emit(dst, pos, b'\\');
    Some(emit(dst, pos, b'0'))
}

/// `escape_octal()`: `\NNN`.
fn escape_octal(dst: &mut [u8], pos: usize, c: u8) -> usize {
    let pos = emit(dst, pos, b'\\');
    let pos = emit(dst, pos, ((c >> 6) & 0x07) + b'0');
    let pos = emit(dst, pos, ((c >> 3) & 0x07) + b'0');
    emit(dst, pos, (c & 0x07) + b'0')
}

/// `escape_hex()`: `\xHH`.
fn escape_hex(dst: &mut [u8], pos: usize, c: u8) -> usize {
    let pos = emit(dst, pos, b'\\');
    let pos = emit(dst, pos, b'x');
    let pos = emit(dst, pos, hex_asc(c >> 4));
    emit(dst, pos, hex_asc(c))
}

/// `string_escape_mem()`: escape the bytes of `src` into `dst` per `flags`,
/// restricted to the `only` set when it is non-empty. Returns the number of
/// bytes that would be written (which may exceed `dst.len()` when `dst` is too
/// small, as the C returns the untruncated length).
pub fn string_escape_mem(src: &[u8], dst: &mut [u8], flags: u32, only: &[u8]) -> usize {
    use crate::ctype::{isascii, isprint};

    let mut p = 0usize;
    let is_dict = !only.is_empty();
    let is_append = flags & ESCAPE_APPEND != 0;

    for &c in src {
        let in_dict = is_dict && only.contains(&c);

        // Pass-through classes, applied before the escaping classes.
        if !(is_append || in_dict) && is_dict {
            p = escape_passthrough(dst, p, c);
            continue;
        }
        if !(is_append && in_dict) && isascii(c) && isprint(c) && flags & ESCAPE_NAP != 0 {
            p = escape_passthrough(dst, p, c);
            continue;
        }
        if !(is_append && in_dict) && isprint(c) && flags & ESCAPE_NP != 0 {
            p = escape_passthrough(dst, p, c);
            continue;
        }
        if !(is_append && in_dict) && isascii(c) && flags & ESCAPE_NA != 0 {
            p = escape_passthrough(dst, p, c);
            continue;
        }

        if flags & ESCAPE_SPACE != 0 {
            if let Some(np) = escape_space(dst, p, c) {
                p = np;
                continue;
            }
        }
        if flags & ESCAPE_SPECIAL != 0 {
            if let Some(np) = escape_special(dst, p, c) {
                p = np;
                continue;
            }
        }
        if flags & ESCAPE_NULL != 0 {
            if let Some(np) = escape_null(dst, p, c) {
                p = np;
                continue;
            }
        }
        // ESCAPE_OCTAL and ESCAPE_HEX always go last.
        if flags & ESCAPE_OCTAL != 0 {
            p = escape_octal(dst, p, c);
            continue;
        }
        if flags & ESCAPE_HEX != 0 {
            p = escape_hex(dst, p, c);
            continue;
        }
        p = escape_passthrough(dst, p, c);
    }
    p
}

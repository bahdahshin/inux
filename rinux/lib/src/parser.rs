// SPDX-License-Identifier: GPL-2.0-only
//! A simple parser for mount and similar options.
//!
//! Ported from `lib/parser.c` and `include/linux/parser.h`.
//!
//! A `substring_t` is a byte slice of the string given to [`match_token`].
//! `match_strdup()` needs the allocator; copy with [`match_strlcpy`].

use crate::ctype::isdigit;
use crate::errno::{EINVAL, ERANGE, Result};
use crate::kstrtox::{kstrtouint, kstrtoull};
use crate::string::{strchr, strcmp, strlen, strncmp};
use crate::vsprintf::{simple_strtol, simple_strtoul};

/// Maximum number of arguments that [`match_token`] will find in a pattern.
pub const MAX_OPT_ARGS: usize = 3;

/// Max size needed by different bases to express a `u64`:
/// hex `"0xFFFFFFFFFFFFFFFF"` is 18, decimal `"18446744073709551615"` is 20,
/// octal `"01777777777777777777777"` is 23.
const NUMBER_BUF_LEN: usize = 24;

/// `struct match_token`: associates an integer with a pattern. A table
/// ends with an entry whose pattern is `None`, which matches anything.
#[derive(Clone, Copy, Debug)]
pub struct MatchToken<'a> {
    pub token: i32,
    pub pattern: Option<&'a [u8]>,
}

#[inline]
fn at(s: &[u8], i: usize) -> u8 {
    s.get(i).copied().unwrap_or(0)
}

#[inline]
fn sub(s: &[u8], i: usize) -> &[u8] {
    s.get(i..).unwrap_or(&[])
}

/// `match_one()`: does `s` match the simple pattern `p`?
///
/// Patterns are literal text with `%s`, `%d`, `%u`, `%o` and `%x`
/// conversions (and `%%`). `%s` may have a maximum length, as in `%10s`.
/// The matched arguments are stored in `args`.
fn match_one<'a>(s: &'a [u8], p: Option<&[u8]>, args: &mut [&'a [u8]; MAX_OPT_ARGS]) -> bool {
    let Some(p) = p else {
        return true;
    };
    let mut si = 0;
    let mut pi = 0;
    let mut argc = 0;

    loop {
        let mut len: i64 = -1;
        let Some(meta) = strchr(sub(p, pi), b'%') else {
            return strcmp(sub(p, pi), sub(s, si)) == 0;
        };

        if strncmp(sub(p, pi), sub(s, si), meta) != 0 {
            return false;
        }

        si += meta;
        pi += meta + 1;

        if isdigit(at(p, pi)) {
            let (v, end) = simple_strtoul(sub(p, pi), 10);
            len = v as i32 as i64;
            pi += end;
        } else if at(p, pi) == b'%' {
            if at(s, si) != b'%' {
                return false;
            }
            si += 1;
            pi += 1;
            continue;
        }

        if argc >= MAX_OPT_ARGS {
            return false;
        }

        let from = si;
        let conv = at(p, pi);
        pi += 1;
        let to = match conv {
            b's' => {
                let str_len = strlen(sub(s, si)) as i64;
                if str_len == 0 {
                    return false;
                }
                if len == -1 || len > str_len {
                    len = str_len;
                }
                si + len as usize
            }
            b'd' | b'u' | b'o' | b'x' => {
                let end = match conv {
                    b'd' => simple_strtol(sub(s, si), 0).1,
                    b'u' => simple_strtoul(sub(s, si), 0).1,
                    b'o' => simple_strtoul(sub(s, si), 8).1,
                    _ => simple_strtoul(sub(s, si), 16).1,
                };
                if end == 0 {
                    return false;
                }
                si + end
            }
            _ => return false,
        };
        args[argc] = &s[from..to];
        si = to;
        argc += 1;
    }
}

/// `match_token()`: finds the first entry of `table` whose pattern
/// matches `s` and returns its token, storing the pattern's arguments in
/// `args`.
///
/// # Panics
///
/// If no pattern matches and the table lacks its `None` terminator; C
/// would read past the table.
pub fn match_token<'a>(
    s: &'a [u8],
    table: &[MatchToken],
    args: &mut [&'a [u8]; MAX_OPT_ARGS],
) -> i32 {
    for p in table {
        if match_one(s, p.pattern, args) {
            return p.token;
        }
    }
    panic!("match_token(): table has no terminating entry");
}

/// `match_number()`: parses a substring as an `int` in `base`.
fn match_number(s: &[u8], base: u32) -> Result<i32> {
    let mut buf = [0u8; NUMBER_BUF_LEN];
    if match_strlcpy(&mut buf, s) >= NUMBER_BUF_LEN {
        return Err(ERANGE);
    }
    let (val, end) = simple_strtol(buf, base);
    if end == 0 {
        Err(EINVAL)
    } else if val < i32::MIN as i64 || val > i32::MAX as i64 {
        Err(ERANGE)
    } else {
        Ok(val as i32)
    }
}

/// `match_u64int()`: parses a substring as a `u64` in `base`.
fn match_u64int(s: &[u8], base: u32) -> Result<u64> {
    let mut buf = [0u8; NUMBER_BUF_LEN];
    if match_strlcpy(&mut buf, s) >= NUMBER_BUF_LEN {
        return Err(ERANGE);
    }
    kstrtoull(&buf[..], base)
}

/// `match_int()`: parses a substring as an `int`; base 0 autodetects.
pub fn match_int(s: &[u8]) -> Result<i32> {
    match_number(s, 0)
}

/// `match_uint()`: parses a substring as a decimal `unsigned int`.
pub fn match_uint(s: &[u8]) -> Result<u32> {
    let mut buf = [0u8; NUMBER_BUF_LEN];
    if match_strlcpy(&mut buf, s) >= NUMBER_BUF_LEN {
        return Err(ERANGE);
    }
    kstrtouint(&buf[..], 10)
}

/// `match_u64()`: parses a substring as a `u64`; base 0 autodetects.
pub fn match_u64(s: &[u8]) -> Result<u64> {
    match_u64int(s, 0)
}

/// `match_octal()`: parses a substring as an octal `int`.
pub fn match_octal(s: &[u8]) -> Result<i32> {
    match_number(s, 8)
}

/// `match_hex()`: parses a substring as a hex `int`.
pub fn match_hex(s: &[u8]) -> Result<i32> {
    match_number(s, 16)
}

/// `match_wildcard()`: does `s` match `pattern`, where `*` matches any
/// run of characters and `?` any one character?
pub fn match_wildcard(pattern: &[u8], s: &[u8]) -> bool {
    let mut si = 0;
    let mut pi = 0;
    let mut str_start = 0;
    let mut pat_start = 0;
    let mut star = false;

    while at(s, si) != 0 {
        match at(pattern, pi) {
            b'?' => {
                si += 1;
                pi += 1;
            }
            b'*' => {
                star = true;
                str_start = si;
                pi += 1;
                if at(pattern, pi) == 0 {
                    return true;
                }
                pat_start = pi;
            }
            c => {
                if at(s, si) == c {
                    si += 1;
                    pi += 1;
                } else {
                    if !star {
                        return false;
                    }
                    str_start += 1;
                    si = str_start;
                    pi = pat_start;
                }
            }
        }
    }

    while at(pattern, pi) == b'*' {
        pi += 1;
    }
    at(pattern, pi) == 0
}

/// `match_strlcpy()`: copies a substring into `dest` as a C string,
/// truncated to `dest.len() - 1` bytes. Returns the substring's length.
pub fn match_strlcpy(dest: &mut [u8], src: &[u8]) -> usize {
    let ret = src.len();
    if !dest.is_empty() {
        let len = if ret >= dest.len() {
            dest.len() - 1
        } else {
            ret
        };
        dest[..len].copy_from_slice(&src[..len]);
        dest[len] = 0;
    }
    ret
}

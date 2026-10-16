// SPDX-License-Identifier: GPL-2.0
//! Converting strings to integers.
//!
//! Ported from `lib/kstrtox.c`, `lib/kstrtox.h` and `include/linux/kstrtox.h`.
//!
//! An integer starts with an optional sign; the unsigned variants do not
//! accept `-`. Radix 0 means autodetection: a leading `0x` means hex, a
//! leading `0` means octal, anything else is decimal. Autodetection hints
//! work after the sign, not before. A single trailing newline is allowed.
//!
//! Inputs follow the C-string convention of [`crate::string`]: parsing
//! stops at the first NUL or the end of the slice. Anything that derefs to
//! bytes works, so `kstrtoint("42", 10)` is fine.
//!
//! The `*_from_user()` variants copy through [`crate::uaccess::UserMemory`]
//! and parse the copy, for the widths that are ported (the 64-bit and 32-bit
//! integers and the boolean); the 16- and 8-bit variants wait for those
//! widths.

use crate::ctype::{_tolower, isdigit, isxdigit};
use crate::errno::{EINVAL, ERANGE, Result};
use crate::math::int_pow;
use crate::uaccess::{
    KSTRTO8_BUF, KSTRTO16_BUF, KSTRTO32_BUF, KSTRTO64_BUF, KSTRTOBOOL_BUF, UserMemory, kstrto_copy,
};

/// Set in the return value of [`parse_integer_limit`] when the value overflowed.
pub const KSTRTOX_OVERFLOW: u32 = 1 << 31;

#[inline]
fn at(s: &[u8], i: usize) -> u8 {
    s.get(i).copied().unwrap_or(0)
}

/// `_parse_integer_fixup_radix()`: resolves base 0 and skips a `0x` prefix.
///
/// Returns the number of prefix bytes to skip.
pub fn parse_integer_fixup_radix(s: &[u8], base: &mut u32) -> usize {
    if *base == 0 {
        if at(s, 0) == b'0' {
            if _tolower(at(s, 1)) == b'x' && isxdigit(at(s, 2)) {
                *base = 16;
            } else {
                *base = 8;
            }
        } else {
            *base = 10;
        }
    }
    if *base == 16 && at(s, 0) == b'0' && _tolower(at(s, 1)) == b'x' {
        2
    } else {
        0
    }
}

/// `_parse_integer_limit()`: parses up to `max_chars` digits in `base`,
/// starting the multiply-accumulate at `init`.
///
/// Returns the number of digits consumed, possibly ORed with
/// [`KSTRTOX_OVERFLOW`], and the value. On overflow the value is `u64::MAX`.
pub fn parse_integer_limit(s: &[u8], base: u32, max_chars: usize, init: u64) -> (u32, u64) {
    let mut res = init;
    let mut overflow = 0;
    let mut rv: u32 = 0;
    while (rv as usize) < max_chars {
        let c = at(s, rv as usize);
        let lc = _tolower(c);
        let val = if c.is_ascii_digit() {
            (c - b'0') as u32
        } else if (b'a'..=b'f').contains(&lc) {
            (lc - b'a' + 10) as u32
        } else {
            break;
        };
        if val >= base {
            break;
        }
        // Check for overflow only when within range of it in base 16.
        if res & (!0u64 << 60) != 0 {
            match res
                .checked_mul(base as u64)
                .and_then(|r| r.checked_add(val as u64))
            {
                Some(r) => res = r,
                None => {
                    res = u64::MAX;
                    overflow = KSTRTOX_OVERFLOW;
                }
            }
        } else {
            res = res * base as u64 + val as u64;
        }
        rv += 1;
    }
    (rv | overflow, res)
}

/// `_parse_integer()`: [`parse_integer_limit`] with no length limit.
pub fn parse_integer(s: &[u8], base: u32) -> (u32, u64) {
    parse_integer_limit(s, base, i32::MAX as usize, 0)
}

fn _kstrtoull(s: &[u8], mut base: u32) -> Result<u64> {
    let s = &s[parse_integer_fixup_radix(s, &mut base).min(s.len())..];
    let (rv, res) = parse_integer(s, base);
    if rv & KSTRTOX_OVERFLOW != 0 {
        return Err(ERANGE);
    }
    if rv == 0 {
        return Err(EINVAL);
    }
    let mut i = rv as usize;
    if at(s, i) == b'\n' {
        i += 1;
    }
    if at(s, i) != 0 {
        return Err(EINVAL);
    }
    Ok(res)
}

fn skip_plus(s: &[u8]) -> &[u8] {
    if at(s, 0) == b'+' { &s[1..] } else { s }
}

/// `kstrtoull()`: string to `u64`. Accepts a leading `+` but not `-`.
///
/// The maximum base is 16. Returns `ERANGE` on overflow and `EINVAL` on a
/// parse error.
pub fn kstrtoull(s: impl AsRef<[u8]>, base: u32) -> Result<u64> {
    _kstrtoull(skip_plus(s.as_ref()), base)
}

/// `kstrtoll()`: string to `i64`. Accepts a leading `+` or `-`.
pub fn kstrtoll(s: impl AsRef<[u8]>, base: u32) -> Result<i64> {
    let s = s.as_ref();
    if at(s, 0) == b'-' {
        let tmp = _kstrtoull(&s[1..], base)?;
        if (tmp.wrapping_neg() as i64) > 0 {
            return Err(ERANGE);
        }
        Ok(tmp.wrapping_neg() as i64)
    } else {
        let tmp = kstrtoull(s, base)?;
        if (tmp as i64) < 0 {
            return Err(ERANGE);
        }
        Ok(tmp as i64)
    }
}

/// `kstrtoul()`: string to `unsigned long`, which is 64 bits on rinux targets.
pub fn kstrtoul(s: impl AsRef<[u8]>, base: u32) -> Result<u64> {
    kstrtoull(s, base)
}

/// `kstrtol()`: string to `long`, which is 64 bits on rinux targets.
pub fn kstrtol(s: impl AsRef<[u8]>, base: u32) -> Result<i64> {
    kstrtoll(s, base)
}

macro_rules! kstrto_narrow {
    ($(#[$doc:meta] $name:ident: $wide:ident -> $ty:ty;)*) => {
        $(
            #[$doc]
            pub fn $name(s: impl AsRef<[u8]>, base: u32) -> Result<$ty> {
                <$ty>::try_from($wide(s, base)?).map_err(|_| ERANGE)
            }
        )*
    };
}

kstrto_narrow! {
    /// `kstrtouint()`: string to `u32`.
    kstrtouint: kstrtoull -> u32;
    /// `kstrtoint()`: string to `i32`.
    kstrtoint: kstrtoll -> i32;
    /// `kstrtou16()`: string to `u16`.
    kstrtou16: kstrtoull -> u16;
    /// `kstrtos16()`: string to `i16`.
    kstrtos16: kstrtoll -> i16;
    /// `kstrtou8()`: string to `u8`.
    kstrtou8: kstrtoull -> u8;
    /// `kstrtos8()`: string to `i8`.
    kstrtos8: kstrtoll -> i8;
}

/// `kstrtou64()`: alias of [`kstrtoull`].
pub fn kstrtou64(s: impl AsRef<[u8]>, base: u32) -> Result<u64> {
    kstrtoull(s, base)
}

/// `kstrtos64()`: alias of [`kstrtoll`].
pub fn kstrtos64(s: impl AsRef<[u8]>, base: u32) -> Result<i64> {
    kstrtoll(s, base)
}

/// `kstrtou32()`: alias of [`kstrtouint`].
pub fn kstrtou32(s: impl AsRef<[u8]>, base: u32) -> Result<u32> {
    kstrtouint(s, base)
}

/// `kstrtos32()`: alias of [`kstrtoint`].
pub fn kstrtos32(s: impl AsRef<[u8]>, base: u32) -> Result<i32> {
    kstrtoint(s, base)
}

/// `kstrtobool()`: reads common user input as a boolean.
///
/// True for a first byte in `EeYyTt1` or for `on`; false for a first byte
/// in `DdNnFf0` or for `off`. Only the first two bytes are looked at.
pub fn kstrtobool(s: impl AsRef<[u8]>) -> Result<bool> {
    let s = s.as_ref();
    match at(s, 0) {
        b'e' | b'E' | b'y' | b'Y' | b't' | b'T' | b'1' => Ok(true),
        b'd' | b'D' | b'n' | b'N' | b'f' | b'F' | b'0' => Ok(false),
        b'o' | b'O' => match at(s, 1) {
            b'n' | b'N' => Ok(true),
            b'f' | b'F' => Ok(false),
            _ => Err(EINVAL),
        },
        _ => Err(EINVAL),
    }
}

fn _kstrtoudec64(s: &[u8], scale: u32) -> Result<u64> {
    let (rv_int, res) = parse_integer(s, 10);
    if rv_int & KSTRTOX_OVERFLOW != 0 {
        return Err(ERANGE);
    }
    let mut i = rv_int as usize;

    if at(s, i) == b'.' {
        i += 1;
    }

    let (rv_frac, mut res) = parse_integer_limit(&s[i.min(s.len())..], 10, scale as usize, res);
    if rv_frac & KSTRTOX_OVERFLOW != 0 {
        return Err(ERANGE);
    }
    i += rv_frac as usize;

    // Look past the digits so that ".5" with scale 0 parses as 0.
    if rv_int == 0 && rv_frac == 0 && !isdigit(at(s, i)) {
        return Err(EINVAL);
    }

    // Truncate digits beyond the scale.
    while isdigit(at(s, i)) {
        i += 1;
    }

    if at(s, i) == b'\n' {
        i += 1;
    }
    if at(s, i) != 0 {
        return Err(EINVAL);
    }

    if res != 0 {
        // log10(2^64) = 19.26
        let missing = scale - rv_frac;
        if missing > 19 {
            return Err(ERANGE);
        }
        res = res.checked_mul(int_pow(10, missing)).ok_or(ERANGE)?;
    }
    Ok(res)
}

/// `kstrtoudec64()`: string to an unsigned scaled decimal.
///
/// With a scale of 3, `"123.45"` gives 123450. Digits beyond the scale are
/// ignored and missing ones count as zero.
pub fn kstrtoudec64(s: impl AsRef<[u8]>, scale: u32) -> Result<u64> {
    _kstrtoudec64(skip_plus(s.as_ref()), scale)
}

/// `kstrtodec64()`: string to a signed scaled decimal.
///
/// With a scale of 4, `"-3.141592"` gives -31415.
pub fn kstrtodec64(s: impl AsRef<[u8]>, scale: u32) -> Result<i64> {
    let s = s.as_ref();
    if at(s, 0) == b'-' {
        let tmp = _kstrtoudec64(&s[1..], scale)?;
        if (tmp.wrapping_neg() as i64) > 0 {
            return Err(ERANGE);
        }
        Ok(tmp.wrapping_neg() as i64)
    } else {
        let tmp = kstrtoudec64(s, scale)?;
        if (tmp as i64) < 0 {
            return Err(ERANGE);
        }
        Ok(tmp as i64)
    }
}

/// `kstrtoull_from_user()`: copies at most 66 bytes from user memory and
/// parses them as [`kstrtoull`] does.
pub fn kstrtoull_from_user(
    user: &impl UserMemory,
    src: usize,
    count: usize,
    base: u32,
) -> Result<u64> {
    let (buf, len) = kstrto_copy::<KSTRTO64_BUF>(user, src, count)?;
    kstrtoull(&buf[..len], base)
}

/// `kstrtoll_from_user()`: as [`kstrtoull_from_user`], signed.
pub fn kstrtoll_from_user(
    user: &impl UserMemory,
    src: usize,
    count: usize,
    base: u32,
) -> Result<i64> {
    let (buf, len) = kstrto_copy::<KSTRTO64_BUF>(user, src, count)?;
    kstrtoll(&buf[..len], base)
}

/// `kstrtoul_from_user()`: the same copy as [`kstrtoull_from_user`].
pub fn kstrtoul_from_user(
    user: &impl UserMemory,
    src: usize,
    count: usize,
    base: u32,
) -> Result<u64> {
    let (buf, len) = kstrto_copy::<KSTRTO64_BUF>(user, src, count)?;
    kstrtoul(&buf[..len], base)
}

/// `kstrtol_from_user()`: the same copy as [`kstrtoll_from_user`].
pub fn kstrtol_from_user(
    user: &impl UserMemory,
    src: usize,
    count: usize,
    base: u32,
) -> Result<i64> {
    let (buf, len) = kstrto_copy::<KSTRTO64_BUF>(user, src, count)?;
    kstrtol(&buf[..len], base)
}

/// `kstrtouint_from_user()`: copies at most 34 bytes, as [`kstrtouint`] takes.
pub fn kstrtouint_from_user(
    user: &impl UserMemory,
    src: usize,
    count: usize,
    base: u32,
) -> Result<u32> {
    let (buf, len) = kstrto_copy::<KSTRTO32_BUF>(user, src, count)?;
    kstrtouint(&buf[..len], base)
}

/// `kstrtoint_from_user()`: the same copy as [`kstrtouint_from_user`].
pub fn kstrtoint_from_user(
    user: &impl UserMemory,
    src: usize,
    count: usize,
    base: u32,
) -> Result<i32> {
    let (buf, len) = kstrto_copy::<KSTRTO32_BUF>(user, src, count)?;
    kstrtoint(&buf[..len], base)
}

/// `kstrtobool_from_user()`: copies at most 3 bytes, enough to tell the
/// accepted words apart.
pub fn kstrtobool_from_user(user: &impl UserMemory, src: usize, count: usize) -> Result<bool> {
    let (buf, len) = kstrto_copy::<KSTRTOBOOL_BUF>(user, src, count)?;
    kstrtobool(&buf[..len])
}

/// `kstrtou16_from_user()`: copies at most 18 bytes, as [`kstrtou16`] takes.
pub fn kstrtou16_from_user(
    user: &impl UserMemory,
    src: usize,
    count: usize,
    base: u32,
) -> Result<u16> {
    let (buf, len) = kstrto_copy::<KSTRTO16_BUF>(user, src, count)?;
    kstrtou16(&buf[..len], base)
}

/// `kstrtos16_from_user()`: the same copy as [`kstrtou16_from_user`].
pub fn kstrtos16_from_user(
    user: &impl UserMemory,
    src: usize,
    count: usize,
    base: u32,
) -> Result<i16> {
    let (buf, len) = kstrto_copy::<KSTRTO16_BUF>(user, src, count)?;
    kstrtos16(&buf[..len], base)
}

/// `kstrtou8_from_user()`: copies at most 10 bytes, as [`kstrtou8`] takes.
pub fn kstrtou8_from_user(
    user: &impl UserMemory,
    src: usize,
    count: usize,
    base: u32,
) -> Result<u8> {
    let (buf, len) = kstrto_copy::<KSTRTO8_BUF>(user, src, count)?;
    kstrtou8(&buf[..len], base)
}

/// `kstrtos8_from_user()`: the same copy as [`kstrtou8_from_user`].
pub fn kstrtos8_from_user(
    user: &impl UserMemory,
    src: usize,
    count: usize,
    base: u32,
) -> Result<i8> {
    let (buf, len) = kstrto_copy::<KSTRTO8_BUF>(user, src, count)?;
    kstrtos8(&buf[..len], base)
}

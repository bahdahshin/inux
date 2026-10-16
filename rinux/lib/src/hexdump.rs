// SPDX-License-Identifier: GPL-2.0-only
//! Hex conversion and hex dumps.
//!
//! Ported from `lib/hexdump.c` and `include/linux/hex.h`. `print_hex_dump()`
//! needs printk and will come with it; [`hex_dump_to_buffer`] produces the
//! same lines.

use crate::ctype::{isascii, isprint};
use crate::errno::{EINVAL, Result};

/// `hex_asc[]`: lower-case hex digits.
pub const HEX_ASC: &[u8; 16] = b"0123456789abcdef";
/// `hex_asc_upper[]`: upper-case hex digits.
pub const HEX_ASC_UPPER: &[u8; 16] = b"0123456789ABCDEF";

/// `hex_asc_lo()`: hex digit for the low nibble.
#[inline]
pub const fn hex_asc_lo(x: u8) -> u8 {
    HEX_ASC[(x & 0x0f) as usize]
}

/// `hex_asc_hi()`: hex digit for the high nibble.
#[inline]
pub const fn hex_asc_hi(x: u8) -> u8 {
    HEX_ASC[((x & 0xf0) >> 4) as usize]
}

/// `hex_asc_upper_lo()`: upper-case hex digit for the low nibble.
#[inline]
pub const fn hex_asc_upper_lo(x: u8) -> u8 {
    HEX_ASC_UPPER[(x & 0x0f) as usize]
}

/// `hex_asc_upper_hi()`: upper-case hex digit for the high nibble.
#[inline]
pub const fn hex_asc_upper_hi(x: u8) -> u8 {
    HEX_ASC_UPPER[((x & 0xf0) >> 4) as usize]
}

/// `hex_byte_pack()`: writes two lower-case hex digits for `byte` at the
/// start of `buf`. Returns the index after them.
#[inline]
pub fn hex_byte_pack(buf: &mut [u8], byte: u8) -> usize {
    buf[0] = hex_asc_hi(byte);
    buf[1] = hex_asc_lo(byte);
    2
}

/// `hex_byte_pack_upper()`: [`hex_byte_pack`] with upper-case digits.
#[inline]
pub fn hex_byte_pack_upper(buf: &mut [u8], byte: u8) -> usize {
    buf[0] = hex_asc_upper_hi(byte);
    buf[1] = hex_asc_upper_lo(byte);
    2
}

/// `hex_to_bin()`: value of a hex digit, or -1 if `ch` is not one.
///
/// This is used to load cryptographic keys, so it has no branches or
/// memory accesses that depend on the data. `(ch - '9' - 1) & ('0' - 1 - ch)`
/// is negative exactly when ch is in `'0'..='9'`; shifting the sign down
/// gives a mask selecting `ch - '0' + 1`. The same is done for letters,
/// with `ch & 0xdf` folding lower case to upper case, and the sum minus 1
/// is the digit value or -1.
pub const fn hex_to_bin(ch: u8) -> i32 {
    let ch = ch as i32;
    let cu = ch & 0xdf;
    let digit = (ch - b'0' as i32 + 1)
        & (((ch - b'9' as i32 - 1) & (b'0' as i32 - 1 - ch)) as u32 >> 8) as i32;
    let alpha = (cu - b'A' as i32 + 11)
        & (((cu - b'F' as i32 - 1) & (b'A' as i32 - 1 - cu)) as u32 >> 8) as i32;
    -1 + digit + alpha
}

/// `hex2bin()`: decodes `2 * dst.len()` hex digits from `src` into `dst`.
pub fn hex2bin(dst: &mut [u8], src: &[u8]) -> Result {
    let mut digits = src.iter().map(|&c| hex_to_bin(c));
    for out in dst {
        let hi = digits.next().unwrap_or(-1);
        if hi < 0 {
            return Err(EINVAL);
        }
        let lo = digits.next().unwrap_or(-1);
        if lo < 0 {
            return Err(EINVAL);
        }
        *out = ((hi << 4) | lo) as u8;
    }
    Ok(())
}

/// `bin2hex()`: encodes `src` as lower-case hex into `dst`, which must
/// hold `2 * src.len()` bytes. Returns the index after the last digit.
pub fn bin2hex(dst: &mut [u8], src: &[u8]) -> usize {
    let mut i = 0;
    for &b in src {
        i += hex_byte_pack(&mut dst[i..], b);
    }
    i
}

/// Formats `" "` (if `sep`) followed by `value` as `width` zero-padded hex
/// digits into `dst` with `snprintf()` semantics: writes as much as fits
/// plus a NUL, and returns the untruncated length.
fn snprintf_hex(dst: &mut [u8], sep: bool, value: u64, width: usize) -> usize {
    let mut tmp = [0u8; 17];
    let mut len = 0;
    if sep {
        tmp[0] = b' ';
        len = 1;
    }
    for k in (0..width).rev() {
        tmp[len] = HEX_ASC[((value >> (4 * k)) & 0xf) as usize];
        len += 1;
    }
    if !dst.is_empty() {
        let n = len.min(dst.len() - 1);
        dst[..n].copy_from_slice(&tmp[..n]);
        dst[n] = 0;
    }
    len
}

/// `hex_dump_to_buffer()`: formats one line of a hex + ASCII dump.
///
/// Dumps at most `rowsize` bytes of `buf` (16 or 32; anything else means 16)
/// in groups of `groupsize` bytes (1, 2, 4 or 8; anything else means 1),
/// followed by the printable ASCII if `ascii` is set. Groups wider than a
/// byte are read in native byte order. For example:
///
/// ```text
/// 40 41 42 43 44 45 46 47 48 49 4a 4b 4c 4d 4e 4f  @ABCDEFGHIJKLMNO
/// ```
///
/// The output in `linebuf` is always NUL-terminated if `linebuf` is not
/// empty. Returns the line length without the NUL; if the line was
/// truncated, the length it would have had.
pub fn hex_dump_to_buffer(
    buf: &[u8],
    rowsize: i32,
    groupsize: i32,
    linebuf: &mut [u8],
    ascii: bool,
) -> i32 {
    let linebuflen = linebuf.len();
    let rowsize = if rowsize != 16 && rowsize != 32 {
        16
    } else {
        rowsize as usize
    };

    // One line at a time.
    let len = buf.len().min(rowsize);
    let mut groupsize = groupsize as usize;
    if !groupsize.is_power_of_two() || groupsize > 8 {
        groupsize = 1;
    }
    // No mixed-size output.
    if len % groupsize != 0 {
        groupsize = 1;
    }

    let ngroups = len / groupsize;
    let ascii_column = rowsize * 2 + rowsize / groupsize + 1;
    let overflow = || -> i32 {
        if ascii {
            (ascii_column + len) as i32
        } else {
            ((groupsize * 2 + 1) * ngroups) as i32 - 1
        }
    };

    if linebuflen == 0 {
        return overflow();
    }

    let mut lx = 0;
    if len != 0 {
        if groupsize > 1 {
            for (j, group) in buf[..len].chunks_exact(groupsize).enumerate() {
                let value = match groupsize {
                    8 => u64::from_ne_bytes(group.try_into().unwrap()),
                    4 => u32::from_ne_bytes(group.try_into().unwrap()) as u64,
                    _ => u16::from_ne_bytes(group.try_into().unwrap()) as u64,
                };
                let ret = snprintf_hex(&mut linebuf[lx..], j != 0, value, groupsize * 2);
                if ret >= linebuflen - lx {
                    return overflow();
                }
                lx += ret;
            }
        } else {
            for &ch in &buf[..len] {
                for out in [hex_asc_hi(ch), hex_asc_lo(ch), b' '] {
                    if linebuflen < lx + 2 {
                        linebuf[lx] = 0;
                        return overflow();
                    }
                    linebuf[lx] = out;
                    lx += 1;
                }
            }
            lx -= 1;
        }

        if ascii {
            while lx < ascii_column {
                if linebuflen < lx + 2 {
                    linebuf[lx] = 0;
                    return overflow();
                }
                linebuf[lx] = b' ';
                lx += 1;
            }
            for &ch in &buf[..len] {
                if linebuflen < lx + 2 {
                    linebuf[lx] = 0;
                    return overflow();
                }
                linebuf[lx] = if isascii(ch) && isprint(ch) { ch } else { b'.' };
                lx += 1;
            }
        }
    }

    linebuf[lx] = 0;
    lx as i32
}

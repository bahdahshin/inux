// SPDX-License-Identifier: GPL-2.0
//! Base64 encoding and decoding in three variants.
//!
//! Ported from `lib/base64.c` and `include/linux/base64.h`.
//!
//! Copyright (c) 2020 Hannes Reinecke, SUSE
//!
//! Based on the base64url routines from fs/crypto/fname.c, modified to
//! support multiple Base64 variants.
//!
//! The C decoder returns -1 for invalid input; [`base64_decode`] returns
//! `None`.

/// `enum base64_variant`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Base64Variant {
    /// `BASE64_STD`: RFC 4648 (standard).
    Std,
    /// `BASE64_URLSAFE`: RFC 4648 (base64url).
    Urlsafe,
    /// `BASE64_IMAP`: RFC 3501.
    Imap,
}

/// `BASE64_CHARS(nbytes)`: encoded length without padding.
pub const fn base64_chars(nbytes: usize) -> usize {
    (nbytes * 4).div_ceil(3)
}

const BASE64_TABLES: [&[u8; 64]; 3] = [
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/",
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_",
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+,",
];

/// `BASE64_REV_INIT()`: maps each byte to its 6-bit value, or -1.
const fn rev_map(ch_62: u8, ch_63: u8) -> [i8; 256] {
    let mut map = [-1i8; 256];
    let mut v = 0;
    while v < 256 {
        let c = v as u8;
        map[v] = match c {
            b'A'..=b'Z' => (c - b'A') as i8,
            b'a'..=b'z' => (c - b'a' + 26) as i8,
            b'0'..=b'9' => (c - b'0' + 52) as i8,
            _ if c == ch_62 => 62,
            _ if c == ch_63 => 63,
            _ => -1,
        };
        v += 1;
    }
    map
}

static BASE64_REV_MAPS: [[i8; 256]; 3] = [
    rev_map(b'+', b'/'),
    rev_map(b'-', b'_'),
    rev_map(b'+', b','),
];

/// `base64_encode()`: encodes `src` into `dst`, which must have room for
/// `4 * ceil(src.len() / 3)` bytes. Appends `=` padding if `padding` is set.
/// The output is not NUL-terminated. Returns its length.
pub fn base64_encode(src: &[u8], dst: &mut [u8], padding: bool, variant: Base64Variant) -> usize {
    let table = BASE64_TABLES[variant as usize];
    let mut cp = 0;
    let mut put = |c: u8| {
        dst[cp] = c;
        cp += 1;
    };

    let mut chunks = src.chunks_exact(3);
    for s in &mut chunks {
        let ac = (s[0] as u32) << 16 | (s[1] as u32) << 8 | s[2] as u32;
        put(table[(ac >> 18) as usize]);
        put(table[((ac >> 12) & 0x3f) as usize]);
        put(table[((ac >> 6) & 0x3f) as usize]);
        put(table[(ac & 0x3f) as usize]);
    }

    match *chunks.remainder() {
        [s0, s1] => {
            let ac = (s0 as u32) << 16 | (s1 as u32) << 8;
            put(table[(ac >> 18) as usize]);
            put(table[((ac >> 12) & 0x3f) as usize]);
            put(table[((ac >> 6) & 0x3f) as usize]);
            if padding {
                put(b'=');
            }
        }
        [s0] => {
            let ac = (s0 as u32) << 16;
            put(table[(ac >> 18) as usize]);
            put(table[((ac >> 12) & 0x3f) as usize]);
            if padding {
                put(b'=');
                put(b'=');
            }
        }
        _ => {}
    }
    cp
}

/// `base64_decode()`: decodes `src` into `dst`, which must have room for
/// `3 * src.len() / 4` bytes. If `padding` is set, the input must be padded
/// with `=` to a multiple of four characters; otherwise it must not be
/// padded. Returns the decoded length, or `None` for invalid input.
pub fn base64_decode(
    src: &[u8],
    dst: &mut [u8],
    mut padding: bool,
    variant: Base64Variant,
) -> Option<usize> {
    let rev = &BASE64_REV_MAPS[variant as usize];
    let r = |c: u8| rev[c as usize] as i32;
    let mut bp = 0;
    let mut s = src;
    let mut srclen = src.len();

    while srclen >= 4 {
        let val = r(s[0]) << 18 | r(s[1]) << 12 | r(s[2]) << 6 | r(s[3]);

        if val < 0 {
            if !padding || srclen != 4 || s[3] != b'=' {
                return None;
            }
            padding = false;
            srclen = if s[2] == b'=' { 2 } else { 3 };
            break;
        }

        dst[bp] = (val >> 16) as u8;
        dst[bp + 1] = (val >> 8) as u8;
        dst[bp + 2] = val as u8;
        bp += 3;

        s = &s[4..];
        srclen -= 4;
    }

    if srclen == 0 {
        return Some(bp);
    }
    if padding || srclen == 1 {
        return None;
    }

    let mut val = (r(s[0]) << 12) | (r(s[1]) << 6);

    if srclen == 2 {
        if val as u32 & 0x8000_03ff != 0 {
            return None;
        }
        dst[bp] = (val >> 10) as u8;
        bp += 1;
    } else {
        val |= r(s[2]);
        if val as u32 & 0x8000_0003 != 0 {
            return None;
        }
        dst[bp] = (val >> 10) as u8;
        dst[bp + 1] = (val >> 2) as u8;
        bp += 2;
    }
    Some(bp)
}

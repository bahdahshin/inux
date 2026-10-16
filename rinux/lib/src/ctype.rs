// SPDX-License-Identifier: GPL-2.0
//! Character classification.
//!
//! Ported from `lib/ctype.c` and `include/linux/ctype.h`.
//!
//! Copyright (C) 1991, 1992  Linus Torvalds
//!
//! Like the C version, this works on bytes and classifies Latin-1 (ISO 8859-1)
//! characters above 0x7f. It does not handle EOF the way the C library does.

/// Upper case.
pub const _U: u8 = 0x01;
/// Lower case.
pub const _L: u8 = 0x02;
/// Digit.
pub const _D: u8 = 0x04;
/// Control character.
pub const _C: u8 = 0x08;
/// Punctuation.
pub const _P: u8 = 0x10;
/// White space (space, line feed, tab).
pub const _S: u8 = 0x20;
/// Hex digit.
pub const _X: u8 = 0x40;
/// Hard space (0x20).
pub const _SP: u8 = 0x80;

const C: u8 = _C;
const CS: u8 = _C | _S;
const P: u8 = _P;
const D: u8 = _D;
const U: u8 = _U;
const L: u8 = _L;
const UX: u8 = _U | _X;
const LX: u8 = _L | _X;
const SSP: u8 = _S | _SP;

/// The `_ctype[]` table: class bits for every byte value.
#[rustfmt::skip]
pub static CTYPE: [u8; 256] = [
    C, C, C, C, C, C, C, C,                                 // 0-7
    C, CS, CS, CS, CS, CS, C, C,                            // 8-15
    C, C, C, C, C, C, C, C,                                 // 16-23
    C, C, C, C, C, C, C, C,                                 // 24-31
    SSP, P, P, P, P, P, P, P,                               // 32-39
    P, P, P, P, P, P, P, P,                                 // 40-47
    D, D, D, D, D, D, D, D,                                 // 48-55
    D, D, P, P, P, P, P, P,                                 // 56-63
    P, UX, UX, UX, UX, UX, UX, U,                           // 64-71
    U, U, U, U, U, U, U, U,                                 // 72-79
    U, U, U, U, U, U, U, U,                                 // 80-87
    U, U, U, P, P, P, P, P,                                 // 88-95
    P, LX, LX, LX, LX, LX, LX, L,                           // 96-103
    L, L, L, L, L, L, L, L,                                 // 104-111
    L, L, L, L, L, L, L, L,                                 // 112-119
    L, L, L, P, P, P, P, C,                                 // 120-127
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,         // 128-143
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,         // 144-159
    SSP, P, P, P, P, P, P, P, P, P, P, P, P, P, P, P,       // 160-175
    P, P, P, P, P, P, P, P, P, P, P, P, P, P, P, P,         // 176-191
    U, U, U, U, U, U, U, U, U, U, U, U, U, U, U, U,         // 192-207
    U, U, U, U, U, U, U, P, U, U, U, U, U, U, U, L,         // 208-223
    L, L, L, L, L, L, L, L, L, L, L, L, L, L, L, L,         // 224-239
    L, L, L, L, L, L, L, P, L, L, L, L, L, L, L, L,         // 240-255
];

#[inline]
const fn ismask(c: u8) -> u8 {
    CTYPE[c as usize]
}

/// Letter or digit.
#[inline]
pub const fn isalnum(c: u8) -> bool {
    ismask(c) & (_U | _L | _D) != 0
}

/// Letter.
#[inline]
pub const fn isalpha(c: u8) -> bool {
    ismask(c) & (_U | _L) != 0
}

/// Control character.
#[inline]
pub const fn iscntrl(c: u8) -> bool {
    ismask(c) & _C != 0
}

/// Printable character other than space.
#[inline]
pub const fn isgraph(c: u8) -> bool {
    ismask(c) & (_P | _U | _L | _D) != 0
}

/// Lower-case letter.
#[inline]
pub const fn islower(c: u8) -> bool {
    ismask(c) & _L != 0
}

/// Printable character, including space.
#[inline]
pub const fn isprint(c: u8) -> bool {
    ismask(c) & (_P | _U | _L | _D | _SP) != 0
}

/// Punctuation.
#[inline]
pub const fn ispunct(c: u8) -> bool {
    ismask(c) & _P != 0
}

/// White space. Returns false for the NUL terminator.
#[inline]
pub const fn isspace(c: u8) -> bool {
    ismask(c) & _S != 0
}

/// Upper-case letter.
#[inline]
pub const fn isupper(c: u8) -> bool {
    ismask(c) & _U != 0
}

/// Hex digit.
#[inline]
pub const fn isxdigit(c: u8) -> bool {
    ismask(c) & (_D | _X) != 0
}

/// 7-bit ASCII.
#[inline]
pub const fn isascii(c: u8) -> bool {
    c <= 0x7f
}

/// Clears the top bit.
#[inline]
pub const fn toascii(c: u8) -> u8 {
    c & 0x7f
}

/// Decimal digit.
#[inline]
pub const fn isdigit(c: u8) -> bool {
    b'0' <= c && c <= b'9'
}

/// Octal digit.
#[inline]
pub const fn isodigit(c: u8) -> bool {
    b'0' <= c && c <= b'7'
}

/// Converts an upper-case letter (ASCII or Latin-1) to lower case.
#[inline]
pub const fn tolower(c: u8) -> u8 {
    if isupper(c) {
        c.wrapping_sub(b'A').wrapping_add(b'a')
    } else {
        c
    }
}

/// Converts a lower-case letter (ASCII or Latin-1) to upper case.
#[inline]
pub const fn toupper(c: u8) -> u8 {
    if islower(c) {
        c.wrapping_sub(b'a').wrapping_add(b'A')
    } else {
        c
    }
}

/// Fast lower-casing for internal use: only correct for letters.
#[inline]
pub const fn _tolower(c: u8) -> u8 {
    c | 0x20
}

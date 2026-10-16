// SPDX-License-Identifier: GPL-2.0-or-later
//! Host tests for `lib/ucs2_string.c`. Linux has no KUnit file for it, so
//! these cover each function's rules and the edge cases in its comments.

use rinux_lib::errno::E2BIG;
use rinux_lib::ucs2_string::{
    ucs2_as_utf8, ucs2_strlen, ucs2_strncmp, ucs2_strnlen, ucs2_strscpy, ucs2_strsize,
    ucs2_utf8size,
};

/// Encode a `&str` as UCS-2 with a trailing NUL, for BMP characters only.
fn ucs2(s: &str) -> Vec<u16> {
    let mut v: Vec<u16> = s.chars().map(|c| c as u16).collect();
    v.push(0);
    v
}

#[test]
fn length_stops_at_the_nul_and_at_maxlength() {
    let s = ucs2("hello");
    assert_eq!(ucs2_strlen(&s), 5);
    assert_eq!(ucs2_strnlen(&s, 3), 3);
    assert_eq!(ucs2_strnlen(&s, 100), 5);
    assert_eq!(ucs2_strlen(&[0, 1, 2]), 0);
    assert_eq!(ucs2_strlen(&[]), 0);
}

#[test]
fn size_is_in_bytes() {
    let s = ucs2("abc");
    assert_eq!(ucs2_strsize(&s, 100), 6);
    assert_eq!(ucs2_strsize(&s, 4), 4);
    // An odd byte count rounds down to whole units.
    assert_eq!(ucs2_strsize(&s, 5), 4);
}

#[test]
fn strscpy_copies_and_terminates() {
    let src = ucs2("abc");
    let mut dst = [0xffffu16; 8];
    assert_eq!(ucs2_strscpy(&mut dst, &src), Ok(3));
    assert_eq!(&dst[..4], &[b'a' as u16, b'b' as u16, b'c' as u16, 0]);
}

#[test]
fn strscpy_truncates_and_reports_e2big() {
    let src = ucs2("Fixed");
    let mut dst = [0xffffu16; 4];
    assert_eq!(ucs2_strscpy(&mut dst, &src), Err(E2BIG));
    // The last unit is the NUL terminator, and the first three are "Fix".
    assert_eq!(dst, [b'F' as u16, b'i' as u16, b'x' as u16, 0]);
}

#[test]
fn strscpy_with_an_empty_destination_fails_untouched() {
    let mut dst: [u16; 0] = [];
    assert_eq!(ucs2_strscpy(&mut dst, &ucs2("x")), Err(E2BIG));
}

#[test]
fn strscpy_fits_exactly_with_room_for_the_nul() {
    let src = ucs2("Fixed");
    let mut dst = [0u16; 6];
    assert_eq!(ucs2_strscpy(&mut dst, &src), Ok(5));
    assert_eq!(dst[5], 0);
}

#[test]
fn strscpy_into_size_one_gives_only_the_nul() {
    let src = ucs2("a");
    let mut dst = [0xffffu16; 1];
    assert_eq!(ucs2_strscpy(&mut dst, &src), Err(E2BIG));
    assert_eq!(dst[0], 0);
}

#[test]
fn compare_orders_units_and_stops_at_nul() {
    assert_eq!(ucs2_strncmp(&ucs2("abc"), &ucs2("abc"), 10), 0);
    assert!(ucs2_strncmp(&ucs2("abc"), &ucs2("abd"), 10) < 0);
    assert!(ucs2_strncmp(&ucs2("abd"), &ucs2("abc"), 10) > 0);
    // Only the first two units are compared.
    assert_eq!(ucs2_strncmp(&ucs2("abx"), &ucs2("aby"), 2), 0);
    // A shorter string sorts first: the NUL is smaller than any unit.
    assert!(ucs2_strncmp(&ucs2("ab"), &ucs2("abc"), 10) < 0);
    // Zero length is always equal.
    assert_eq!(ucs2_strncmp(&ucs2("a"), &ucs2("z"), 0), 0);
}

#[test]
fn utf8_size_counts_one_two_and_three_byte_units() {
    // 'A' is 1 byte, U+00E9 is 2, U+20AC (euro) is 3.
    let s = [0x41u16, 0xe9, 0x20ac, 0];
    assert_eq!(ucs2_utf8size(&s), 6);
    assert_eq!(ucs2_utf8size(&[0]), 0);
    // The boundaries: 0x7f is one byte, 0x80 two, 0x7ff two, 0x800 three.
    assert_eq!(ucs2_utf8size(&[0x7f, 0]), 1);
    assert_eq!(ucs2_utf8size(&[0x80, 0]), 2);
    assert_eq!(ucs2_utf8size(&[0x7ff, 0]), 2);
    assert_eq!(ucs2_utf8size(&[0x800, 0]), 3);
}

#[test]
fn conversion_matches_the_utf8_encoding() {
    // "A" then U+00E9 "é" then U+20AC "€".
    let src = [0x41u16, 0xe9, 0x20ac, 0];
    let mut dest = [0u8; 16];
    let n = ucs2_as_utf8(&mut dest, &src, 16);
    assert_eq!(n, 6);
    assert_eq!(&dest[..n], "A\u{e9}\u{20ac}".as_bytes());
    assert_eq!(dest[n], 0, "NUL written when there is room");
}

#[test]
fn conversion_never_splits_a_character() {
    // A 3-byte unit needs 3 bytes; with 2 bytes left it must be skipped.
    let src = [0x41u16, 0x20ac, 0];
    let mut dest = [0xffu8; 4];
    let n = ucs2_as_utf8(&mut dest, &src, 3);
    assert_eq!(n, 1);
    assert_eq!(dest[0], b'A');
    assert_eq!(dest[1], 0, "NUL goes after the last whole character");
}

#[test]
fn conversion_into_a_zero_budget_writes_nothing() {
    let src = [0x41u16, 0];
    let mut dest = [0xffu8; 2];
    assert_eq!(ucs2_as_utf8(&mut dest, &src, 0), 0);
    assert_eq!(dest, [0xff, 0xff]);
}

#[test]
fn conversion_of_a_two_byte_unit_round_trips_through_std() {
    for c in [0x80u16, 0x7ff, 0x800, 0xd7ff, 0xe000, 0xfffd] {
        let Some(ch) = char::from_u32(c as u32) else {
            continue;
        };
        let src = [c, 0];
        let mut dest = [0u8; 4];
        let n = ucs2_as_utf8(&mut dest, &src, 4);
        assert_eq!(&dest[..n], ch.to_string().as_bytes(), "unit {c:#x}");
    }
}

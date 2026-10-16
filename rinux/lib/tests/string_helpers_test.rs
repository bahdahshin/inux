// SPDX-License-Identifier: GPL-2.0-or-later
//! Host tests for `lib/string_helpers.c`: the size formatter, the unescaper
//! and the escaper. The expectations are the C output for the same inputs.

use rinux_lib::string_helpers::{
    ESCAPE_ANY_NP, ESCAPE_HEX, ESCAPE_NP, ESCAPE_OCTAL, ESCAPE_SPACE, ESCAPE_SPECIAL,
    STRING_UNITS_2, STRING_UNITS_NO_BYTES, STRING_UNITS_NO_SPACE, UNESCAPE_ANY, UNESCAPE_HEX,
    UNESCAPE_OCTAL, UNESCAPE_SPACE, UNESCAPE_SPECIAL, string_escape_mem, string_get_size,
    string_unescape,
};

/// Format a size and return the text, checking the length matches.
fn size(bytes: u64, blk: u64, units: u32) -> String {
    let mut buf = [0u8; 64];
    let n = string_get_size(bytes, blk, units, &mut buf);
    let end = buf.iter().position(|&b| b == 0).unwrap();
    assert_eq!(n, end, "length is the text length");
    String::from_utf8(buf[..end].to_vec()).unwrap()
}

#[test]
fn small_sizes_are_plain_bytes() {
    assert_eq!(size(0, 1, 0), "0 B");
    assert_eq!(size(500, 1, 0), "500 B");
}

#[test]
fn si_units_round_to_three_digits() {
    assert_eq!(size(1000, 1, 0), "1.00 kB");
    assert_eq!(size(1500, 1, 0), "1.50 kB");
}

#[test]
fn binary_units_use_the_ki_prefixes() {
    assert_eq!(size(1024, 1, STRING_UNITS_2), "1.00 KiB");
}

#[test]
fn the_block_size_multiplies_in() {
    // 4 blocks of 512 bytes is 2048 bytes.
    assert_eq!(size(4, 512, STRING_UNITS_2), "2.00 KiB");
}

#[test]
fn flags_drop_the_space_and_the_byte_suffix() {
    assert_eq!(size(1000, 1, STRING_UNITS_NO_SPACE), "1.00kB");
    assert_eq!(
        size(1000, 1, STRING_UNITS_NO_SPACE | STRING_UNITS_NO_BYTES),
        "1.00k"
    );
}

#[test]
fn a_zero_block_size_is_zero() {
    assert_eq!(size(1000, 0, 0), "0 B");
}

#[test]
fn the_short_buffer_is_truncated_and_terminated() {
    let mut buf = [0xffu8; 4];
    let n = string_get_size(1000, 1, 0, &mut buf);
    assert_eq!(n, 7, "returns the full length");
    assert_eq!(&buf, b"1.0\0");
}

#[test]
fn unescape_decodes_each_enabled_class() {
    let mut dst = [0u8; 16];
    let n = string_unescape(br"a\nb", &mut dst, UNESCAPE_SPACE);
    assert_eq!(&dst[..n], b"a\nb");

    let n = string_unescape(br"\x41\x4", &mut dst, UNESCAPE_HEX);
    assert_eq!(&dst[..n], b"A\x04");

    let n = string_unescape(br"\101", &mut dst, UNESCAPE_OCTAL);
    assert_eq!(&dst[..n], b"A");

    let n = string_unescape(br#"\"\\"#, &mut dst, UNESCAPE_SPECIAL);
    assert_eq!(&dst[..n], b"\"\\");
}

#[test]
fn an_escape_whose_class_is_off_is_kept() {
    let mut dst = [0u8; 16];
    let n = string_unescape(br"\n", &mut dst, UNESCAPE_HEX);
    assert_eq!(&dst[..n], br"\n");
}

#[test]
fn unescape_with_all_classes() {
    let mut dst = [0u8; 16];
    let n = string_unescape(br"x\ty\x20\101", &mut dst, UNESCAPE_ANY);
    assert_eq!(&dst[..n], b"x\ty A");
}

#[test]
fn unescape_stops_at_the_destination_size() {
    let mut dst = [0u8; 3];
    let n = string_unescape(b"abcdef", &mut dst, 0);
    assert_eq!(&dst, b"ab\0");
    assert_eq!(n, 2);
}

#[test]
fn escape_space_and_special_use_backslash_letters() {
    let mut dst = [0u8; 16];
    let n = string_escape_mem(b"a\n\"", &mut dst, ESCAPE_SPACE | ESCAPE_SPECIAL, b"");
    assert_eq!(&dst[..n], br#"a\n\""#);
}

#[test]
fn escape_octal_and_hex_are_the_last_resort() {
    let mut dst = [0u8; 16];
    let n = string_escape_mem(&[0x01], &mut dst, ESCAPE_OCTAL, b"");
    assert_eq!(&dst[..n], br"\001");

    let n = string_escape_mem(&[0x01], &mut dst, ESCAPE_HEX, b"");
    assert_eq!(&dst[..n], br"\x01");
}

#[test]
fn np_passes_printable_characters_through() {
    let mut dst = [0u8; 16];
    let n = string_escape_mem(b"a\x01", &mut dst, ESCAPE_NP | ESCAPE_HEX, b"");
    assert_eq!(&dst[..n], br"a\x01");
}

#[test]
fn escape_any_np_quotes_everything_unprintable() {
    let mut dst = [0u8; 32];
    let n = string_escape_mem(b"a\t\0z", &mut dst, ESCAPE_ANY_NP, b"");
    assert_eq!(&dst[..n], br"a\t\0z");
}

#[test]
fn escape_reports_the_full_length_when_truncated() {
    let mut dst = [0u8; 2];
    let n = string_escape_mem(&[0x01], &mut dst, ESCAPE_HEX, b"");
    assert_eq!(n, 4);
    assert_eq!(&dst, br"\x");
}

#[test]
fn escape_then_unescape_round_trips() {
    let src = b"tab\tquote\"back\\x01end";
    let mut escaped = [0u8; 128];
    let n = string_escape_mem(
        src,
        &mut escaped,
        ESCAPE_SPACE | ESCAPE_SPECIAL | ESCAPE_OCTAL,
        b"",
    );
    let mut back = [0u8; 64];
    let m = string_unescape(&escaped[..n], &mut back, UNESCAPE_ANY);
    assert_eq!(&back[..m], src);
}

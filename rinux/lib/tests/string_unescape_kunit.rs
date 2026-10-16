// SPDX-License-Identifier: GPL-2.0
//! Tests for `string_unescape`, in the spirit of the escape tests in
//! `lib/tests/string_helpers_kunit.c`.

use rinux_lib::string::*;

/// Unescape `src` with `flags` and return the decoded bytes.
fn un(src: &[u8], flags: u32) -> Vec<u8> {
    let mut dst = vec![0u8; src.len() + 1];
    let n = string_unescape(src, &mut dst, flags);
    dst.truncate(n);
    dst
}

#[test]
fn space_escapes() {
    assert_eq!(un(b"a\\nb\\tc", UNESCAPE_SPACE), b"a\nb\tc");
    assert_eq!(un(b"\\r\\v\\f", UNESCAPE_SPACE), b"\r\x0b\x0c");
}

#[test]
fn special_escapes() {
    assert_eq!(
        un(b"\\\\ \\\" \\a \\e", UNESCAPE_SPECIAL),
        b"\\ \" \x07 \x1b"
    );
}

#[test]
fn hex_escapes() {
    assert_eq!(un(b"\\x41\\x42", UNESCAPE_HEX), b"AB");
    // A single hex digit is allowed.
    assert_eq!(un(b"\\x9z", UNESCAPE_HEX), b"\x09z");
}

#[test]
fn octal_escapes() {
    assert_eq!(un(b"\\101\\102", UNESCAPE_OCTAL), b"AB"); // 0101=65='A'
    assert_eq!(un(b"\\0", UNESCAPE_OCTAL), b"\0");
    // At most three octal digits.
    assert_eq!(un(b"\\1011", UNESCAPE_OCTAL), b"A1");
}

#[test]
fn any_and_passthrough() {
    assert_eq!(un(b"tab\\tnl\\nx\\x21", UNESCAPE_ANY), b"tab\tnl\nx!");
    // An unknown escape keeps the backslash.
    assert_eq!(un(b"\\q", UNESCAPE_ANY), b"\\q");
    // A trailing backslash is kept as-is.
    assert_eq!(un(b"end\\", UNESCAPE_ANY), b"end\\");
    // Plain text is copied unchanged.
    assert_eq!(un(b"hello world", UNESCAPE_ANY), b"hello world");
}

#[test]
fn respects_dst_bound() {
    let src = b"\\n\\n\\n\\n";
    let mut dst = [0u8; 2];
    let n = string_unescape(src, &mut dst, UNESCAPE_SPACE);
    assert!(n <= 2);
    assert_eq!(&dst[..n], &b"\n\n"[..n]);
}

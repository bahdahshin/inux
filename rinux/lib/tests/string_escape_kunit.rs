// SPDX-License-Identifier: GPL-2.0
//! Tests for `string_escape_mem`, including a round-trip through
//! `string_unescape`.

use rinux_lib::string::{
    ESCAPE_ANY, ESCAPE_APPEND, ESCAPE_HEX, ESCAPE_NP, ESCAPE_NULL, ESCAPE_OCTAL, ESCAPE_SPACE,
    ESCAPE_SPECIAL, UNESCAPE_ANY, string_escape_mem, string_unescape,
};

fn esc(src: &[u8], flags: u32, only: &[u8]) -> Vec<u8> {
    let mut buf = vec![0u8; 1024];
    let n = string_escape_mem(src, &mut buf, flags, only);
    assert!(n <= buf.len());
    buf.truncate(n);
    buf
}

#[test]
fn escape_classes() {
    assert_eq!(esc(b"a\nb\tc", ESCAPE_SPACE, b""), b"a\\nb\\tc");
    assert_eq!(esc(b"a\\b\"c", ESCAPE_SPECIAL, b""), b"a\\\\b\\\"c");
    assert_eq!(esc(&[0], ESCAPE_NULL, b""), b"\\0");
    assert_eq!(esc(&[1], ESCAPE_OCTAL, b""), b"\\001");
    assert_eq!(esc(&[0xff], ESCAPE_HEX, b""), b"\\xff");

    // ESCAPE_NP passes printables through; non-printables take the octal path.
    assert_eq!(esc(b"ab", ESCAPE_NP | ESCAPE_OCTAL, b""), b"ab");
    assert_eq!(esc(&[0x01, b'a'], ESCAPE_NP | ESCAPE_OCTAL, b""), b"\\001a");
}

#[test]
fn only_and_append() {
    // Without APPEND, ESCAPE_NP passes every printable, even dict members.
    assert_eq!(esc(b"abc", ESCAPE_NP | ESCAPE_OCTAL, b"b"), b"abc");
    // With APPEND, dict members are forced out of the pass-through class.
    // 'b' is 0x62 = octal 142.
    assert_eq!(
        esc(b"abc", ESCAPE_NP | ESCAPE_OCTAL | ESCAPE_APPEND, b"b"),
        b"a\\142c"
    );
}

#[test]
fn truncation_returns_full_length() {
    // Two newlines escape to four bytes; the return value is the untruncated
    // length even when the buffer is too small.
    let mut small = [0u8; 2];
    let n = string_escape_mem(b"\n\n", &mut small, ESCAPE_SPACE, b"");
    assert_eq!(n, 4);
    assert_eq!(&small, b"\\n"); // only the part that fit was written
}

#[test]
fn roundtrip_all_bytes() {
    // ESCAPE_ANY escapes every byte (printables via octal), so unescaping with
    // UNESCAPE_ANY must recover the original byte exactly.
    for b in 0u8..=255 {
        let e = esc(&[b], ESCAPE_ANY, b"");
        let mut out = [0u8; 8];
        let n = string_unescape(&e, &mut out, UNESCAPE_ANY);
        assert_eq!(&out[..n], &[b], "byte {b:#04x} escaped to {e:?}");
    }
}

#[test]
fn roundtrip_mixed_string() {
    let src: Vec<u8> = (0..200u32).map(|i| (i * 37 + 11) as u8).collect();
    let e = esc(&src, ESCAPE_ANY, b"");
    let mut out = vec![0u8; src.len()];
    let n = string_unescape(&e, &mut out, UNESCAPE_ANY);
    assert_eq!(&out[..n], &src[..]);
}

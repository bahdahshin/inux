// SPDX-License-Identifier: GPL-2.0
//! Port of `lib/tests/base64_kunit.c`.
//!
//! Copyright (c) 2025, Guan-Chun Wu <409411716@gms.tku.edu.tw>
//!
//! The performance cases keep their round-trip check on pseudo-random
//! data; the timing is not ported.

use rinux_lib::base64::{Base64Variant, base64_decode, base64_encode};
use rinux_lib::random32::RndState;

use Base64Variant::{Imap, Std, Urlsafe};

fn run_perf_and_check(size: usize, variant: Base64Variant) {
    let outlen = size.div_ceil(3) * 4;
    let mut input = vec![0u8; size];
    let mut enc = vec![0u8; outlen];
    let mut decoded = vec![0u8; size];

    RndState::new(size as u64).fill_bytes(&mut input);
    let enc_len = base64_encode(&input, &mut enc, true, variant);
    let dec_len = base64_decode(&enc[..enc_len], &mut decoded, true, variant);

    // Correctness sanity check.
    assert_eq!(dec_len, Some(size));
    assert_eq!(decoded, input);
}

#[test]
fn base64_performance_tests() {
    // Run on STD variant only.
    run_perf_and_check(64, Std);
    run_perf_and_check(1024, Std);
}

#[track_caller]
fn expect_encode_ok(src: &[u8], expected: &str, padding: bool, variant: Base64Variant) {
    let mut buf = [0u8; 128];
    let encoded_len = base64_encode(src, &mut buf, padding, variant);

    assert_eq!(encoded_len, expected.len());
    assert_eq!(&buf[..encoded_len], expected.as_bytes());
}

#[track_caller]
fn expect_decode_ok(src: &str, expected: &[u8], padding: bool, variant: Base64Variant) {
    let mut buf = [0u8; 128];
    let decoded_len = base64_decode(src.as_bytes(), &mut buf, padding, variant);

    assert_eq!(decoded_len, Some(expected.len()));
    assert_eq!(&buf[..expected.len()], expected);
}

#[track_caller]
fn expect_decode_err(src: &[u8], padding: bool, variant: Base64Variant) {
    let mut buf = [0u8; 64];
    assert_eq!(base64_decode(src, &mut buf, padding, variant), None);
}

#[test]
fn base64_std_encode_tests() {
    // With padding.
    expect_encode_ok(b"", "", true, Std);
    expect_encode_ok(b"f", "Zg==", true, Std);
    expect_encode_ok(b"fo", "Zm8=", true, Std);
    expect_encode_ok(b"foo", "Zm9v", true, Std);
    expect_encode_ok(b"foob", "Zm9vYg==", true, Std);
    expect_encode_ok(b"fooba", "Zm9vYmE=", true, Std);
    expect_encode_ok(b"foobar", "Zm9vYmFy", true, Std);

    // Extra cases with padding.
    expect_encode_ok(b"Hello, world!", "SGVsbG8sIHdvcmxkIQ==", true, Std);
    expect_encode_ok(
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZ",
        "QUJDREVGR0hJSktMTU5PUFFSU1RVVldYWVo=",
        true,
        Std,
    );
    expect_encode_ok(
        b"abcdefghijklmnopqrstuvwxyz",
        "YWJjZGVmZ2hpamtsbW5vcHFyc3R1dnd4eXo=",
        true,
        Std,
    );
    expect_encode_ok(b"0123456789+/", "MDEyMzQ1Njc4OSsv", true, Std);

    // Without padding.
    expect_encode_ok(b"", "", false, Std);
    expect_encode_ok(b"f", "Zg", false, Std);
    expect_encode_ok(b"fo", "Zm8", false, Std);
    expect_encode_ok(b"foo", "Zm9v", false, Std);
    expect_encode_ok(b"foob", "Zm9vYg", false, Std);
    expect_encode_ok(b"fooba", "Zm9vYmE", false, Std);
    expect_encode_ok(b"foobar", "Zm9vYmFy", false, Std);

    // Extra cases without padding.
    expect_encode_ok(b"Hello, world!", "SGVsbG8sIHdvcmxkIQ", false, Std);
    expect_encode_ok(
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZ",
        "QUJDREVGR0hJSktMTU5PUFFSU1RVVldYWVo",
        false,
        Std,
    );
    expect_encode_ok(
        b"abcdefghijklmnopqrstuvwxyz",
        "YWJjZGVmZ2hpamtsbW5vcHFyc3R1dnd4eXo",
        false,
        Std,
    );
    expect_encode_ok(b"0123456789+/", "MDEyMzQ1Njc4OSsv", false, Std);
}

#[test]
fn base64_std_decode_tests() {
    // With padding.
    expect_decode_ok("", b"", true, Std);
    expect_decode_ok("Zg==", b"f", true, Std);
    expect_decode_ok("Zm8=", b"fo", true, Std);
    expect_decode_ok("Zm9v", b"foo", true, Std);
    expect_decode_ok("Zm9vYg==", b"foob", true, Std);
    expect_decode_ok("Zm9vYmE=", b"fooba", true, Std);
    expect_decode_ok("Zm9vYmFy", b"foobar", true, Std);
    expect_decode_ok("SGVsbG8sIHdvcmxkIQ==", b"Hello, world!", true, Std);
    expect_decode_ok(
        "QUJDREVGR0hJSktMTU5PUFFSU1RVVldYWVo=",
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZ",
        true,
        Std,
    );
    expect_decode_ok(
        "YWJjZGVmZ2hpamtsbW5vcHFyc3R1dnd4eXo=",
        b"abcdefghijklmnopqrstuvwxyz",
        true,
        Std,
    );

    // Error cases.
    expect_decode_err(b"Zg=!", true, Std);
    expect_decode_err(b"Zm$=", true, Std);
    expect_decode_err(b"Z===", true, Std);
    expect_decode_err(b"Zg", true, Std);
    expect_decode_err(b"Zm9v====", true, Std);
    expect_decode_err(b"Zm==A", true, Std);
    expect_decode_err(b"Zg\0=", true, Std);

    // Without padding.
    expect_decode_ok("", b"", false, Std);
    expect_decode_ok("Zg", b"f", false, Std);
    expect_decode_ok("Zm8", b"fo", false, Std);
    expect_decode_ok("Zm9v", b"foo", false, Std);
    expect_decode_ok("Zm9vYg", b"foob", false, Std);
    expect_decode_ok("Zm9vYmE", b"fooba", false, Std);
    expect_decode_ok("Zm9vYmFy", b"foobar", false, Std);
    expect_decode_ok("TWFu", b"Man", false, Std);
    expect_decode_ok("SGVsbG8sIHdvcmxkIQ", b"Hello, world!", false, Std);
    expect_decode_ok(
        "QUJDREVGR0hJSktMTU5PUFFSU1RVVldYWVo",
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZ",
        false,
        Std,
    );
    expect_decode_ok(
        "YWJjZGVmZ2hpamtsbW5vcHFyc3R1dnd4eXo",
        b"abcdefghijklmnopqrstuvwxyz",
        false,
        Std,
    );
    expect_decode_ok("MDEyMzQ1Njc4OSsv", b"0123456789+/", false, Std);

    // Error cases.
    expect_decode_err(b"Zg=!", false, Std);
    expect_decode_err(b"Zm$=", false, Std);
    expect_decode_err(b"Z===", false, Std);
    expect_decode_err(b"Zg=", false, Std);
    expect_decode_err(b"Zm9v====", false, Std);
    expect_decode_err(&b"Zm==v"[..4], false, Std);
    expect_decode_err(b"Zg\0=", false, Std);
}

#[test]
fn base64_variant_tests() {
    let sample1: [u8; 5] = [0x00, 0xfb, 0xff, 0x7f, 0x80];
    let mut std_buf = [0u8; 128];
    let mut url_buf = [0u8; 128];
    let mut imap_buf = [0u8; 128];
    let mut back = [0u8; 128];

    let n_std = base64_encode(&sample1, &mut std_buf, false, Std);
    let n_url = base64_encode(&sample1, &mut url_buf, false, Urlsafe);

    for c in &mut std_buf[..n_std] {
        if *c == b'+' {
            *c = b'-';
        } else if *c == b'/' {
            *c = b'_';
        }
    }
    assert_eq!(&std_buf[..n_std], &url_buf[..n_url]);

    let m = base64_decode(&url_buf[..n_url], &mut back, false, Urlsafe);
    assert_eq!(m, Some(sample1.len()));
    assert_eq!(&back[..sample1.len()], &sample1);

    let n_std = base64_encode(&sample1, &mut std_buf, false, Std);
    let n_imap = base64_encode(&sample1, &mut imap_buf, false, Imap);

    for c in &mut std_buf[..n_std] {
        if *c == b'/' {
            *c = b',';
        }
    }
    assert_eq!(&std_buf[..n_std], &imap_buf[..n_imap]);

    let m = base64_decode(&imap_buf[..n_imap], &mut back, false, Imap);
    assert_eq!(m, Some(sample1.len()));
    assert_eq!(&back[..sample1.len()], &sample1);

    let bad = b"Zg==";
    let mut tmp = [0u8; 8];
    assert_eq!(base64_decode(bad, &mut tmp, false, Urlsafe), None);
    assert_eq!(base64_decode(bad, &mut tmp, false, Imap), None);
}

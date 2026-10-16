// SPDX-License-Identifier: GPL-2.0
//! MD5 known-answer tests: the RFC 1321 test suite plus a streaming check.

use rinux_lib::md5::{Md5, md5};

fn hex(d: &[u8; 16]) -> String {
    use std::fmt::Write;
    let mut s = String::new();
    for b in d {
        let _ = write!(s, "{b:02x}");
    }
    s
}

#[test]
fn rfc1321_suite() {
    let cases: &[(&[u8], &str)] = &[
        (b"", "d41d8cd98f00b204e9800998ecf8427e"),
        (b"a", "0cc175b9c0f1b6a831c399e269772661"),
        (b"abc", "900150983cd24fb0d6963f7d28e17f72"),
        (b"message digest", "f96b697d7cb7938d525a2f31aaf161d0"),
        (
            b"abcdefghijklmnopqrstuvwxyz",
            "c3fcd3d76192e4007dfb496cca67e13b",
        ),
        (
            b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789",
            "d174ab98d277d9f5a5611c2c9f419d9f",
        ),
        (
            b"12345678901234567890123456789012345678901234567890123456789012345678901234567890",
            "57edf4a22be3c955ac49da2e2107b67a",
        ),
    ];
    for (msg, want) in cases {
        assert_eq!(
            hex(&md5(msg)),
            *want,
            "md5 of {:?}",
            core::str::from_utf8(msg)
        );
    }
}

#[test]
fn streaming_matches_oneshot() {
    let data: Vec<u8> = (0..5000u32).map(|i| (i * 11) as u8).collect();
    let mut h = Md5::new();
    for piece in data.chunks(17) {
        h.update(piece);
    }
    assert_eq!(h.finalize(), md5(&data));
}

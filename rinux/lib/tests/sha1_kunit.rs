// SPDX-License-Identifier: GPL-2.0
//! SHA-1 known-answer tests.
//!
//! The classic published vectors (FIPS 180-1 / RFC 3174) plus a streaming
//! check, against the ported `sha1()` and the incremental `Sha1` API.

use rinux_lib::sha1::{Sha1, sha1};

fn hex(d: &[u8; 20]) -> String {
    use std::fmt::Write;
    let mut s = String::new();
    for b in d {
        let _ = write!(s, "{b:02x}");
    }
    s
}

#[test]
fn vec_empty() {
    assert_eq!(hex(&sha1(b"")), "da39a3ee5e6b4b0d3255bfef95601890afd80709");
}

#[test]
fn vec_abc() {
    assert_eq!(
        hex(&sha1(b"abc")),
        "a9993e364706816aba3e25717850c26c9cd0d89d"
    );
}

#[test]
fn vec_two_block() {
    let msg = b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq";
    assert_eq!(hex(&sha1(msg)), "84983e441c3bd26ebaae4aa1f95129e5e54670f1");
}

#[test]
fn vec_million_a() {
    let mut h = Sha1::new();
    let chunk = [b'a'; 1000];
    for _ in 0..1000 {
        h.update(&chunk);
    }
    assert_eq!(
        hex(&h.finalize()),
        "34aa973cd4c4daa4f61eeb2bdbad27316534016f"
    );
}

#[test]
fn streaming_matches_oneshot() {
    let data: Vec<u8> = (0..2000u32).map(|i| (i * 3) as u8).collect();
    let mut h = Sha1::new();
    for piece in data.chunks(7) {
        h.update(piece);
    }
    assert_eq!(h.finalize(), sha1(&data));
}

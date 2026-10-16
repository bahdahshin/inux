// SPDX-License-Identifier: GPL-2.0
//! SHA-256 known-answer tests.
//!
//! The FIPS 180-4 example digests plus a multi-block and a streaming case,
//! checking the ported `sha256()` and the incremental `Sha256` API against
//! published vectors.

use rinux_lib::sha256::{Sha256, sha256};

/// Formats a digest as lowercase hex.
fn hex(d: &[u8; 32]) -> String {
    use std::fmt::Write;
    let mut s = String::new();
    for b in d {
        let _ = write!(s, "{b:02x}");
    }
    s
}

#[test]
fn fips_empty() {
    assert_eq!(
        hex(&sha256(b"")),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
}

#[test]
fn fips_abc() {
    assert_eq!(
        hex(&sha256(b"abc")),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
}

#[test]
fn fips_two_block() {
    // 56 bytes: forces padding into a second block.
    let msg = b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq";
    assert_eq!(
        hex(&sha256(msg)),
        "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
    );
}

#[test]
fn million_a() {
    // One million 'a's: the classic multi-block vector.
    let mut h = Sha256::new();
    let chunk = [b'a'; 1000];
    for _ in 0..1000 {
        h.update(&chunk);
    }
    assert_eq!(
        hex(&h.finalize()),
        "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0"
    );
}

#[test]
fn streaming_matches_oneshot() {
    // Feeding the data in odd-sized pieces must match the one-shot hash.
    let data: Vec<u8> = (0..1234u32).map(|i| (i * 7) as u8).collect();
    let mut h = Sha256::new();
    for piece in data.chunks(13) {
        h.update(piece);
    }
    assert_eq!(h.finalize(), sha256(&data));
}

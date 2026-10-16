// SPDX-License-Identifier: GPL-2.0-or-later
//! Host tests for Poly1305, against the RFC 8439 vector and the empty message.

use rinux_lib::crypto::poly1305::poly1305;

fn unhex(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}

#[test]
fn rfc_8439_vector() {
    let key: [u8; 32] = unhex("85d6be7857556d337f4452fe42d506a80103808afb0db2fd4abff6af4149f51b")
        .try_into()
        .unwrap();
    let tag = poly1305(&key, b"Cryptographic Forum Research Group");
    assert_eq!(tag.to_vec(), unhex("a8061dc1305136c6c22b8baf0c0127a9"));
}

#[test]
fn empty_message_gives_the_s_half_of_the_key() {
    let key: [u8; 32] = core::array::from_fn(|i| i as u8);
    let tag = poly1305(&key, &[]);
    assert_eq!(tag.to_vec(), key[16..32].to_vec());
}


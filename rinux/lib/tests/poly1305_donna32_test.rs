// SPDX-License-Identifier: GPL-2.0-or-later
//! Host tests for the 26-bit Poly1305 core. The tags must match the 44-bit
//! core for every message length, and the RFC 8439 vector.

use rinux_lib::crypto::poly1305::poly1305;
use rinux_lib::crypto::poly1305_donna32::poly1305_tag;

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
    let tag = poly1305_tag(&key, b"Cryptographic Forum Research Group");
    assert_eq!(tag.to_vec(), unhex("a8061dc1305136c6c22b8baf0c0127a9"));
}

#[test]
fn empty_message_gives_the_s_half_of_the_key() {
    let key: [u8; 32] = core::array::from_fn(|i| i as u8);
    assert_eq!(poly1305_tag(&key, &[]).to_vec(), key[16..32].to_vec());
}

#[test]
fn matches_the_44_bit_core_for_every_length() {
    // A key with every byte value present, so the clamp and the limbs see
    // all bit patterns, and messages of each length from 0 to 300 bytes.
    let key: [u8; 32] = core::array::from_fn(|i| (i as u8).wrapping_mul(37).wrapping_add(11));
    let msg: Vec<u8> = (0..300u32)
        .map(|i| (i.wrapping_mul(131) ^ 0x5a) as u8)
        .collect();
    for len in 0..=msg.len() {
        assert_eq!(
            poly1305_tag(&key, &msg[..len]),
            poly1305(&key, &msg[..len]),
            "length {len}"
        );
    }
}

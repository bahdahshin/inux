// SPDX-License-Identifier: GPL-2.0-or-later
//! Host tests for NH, with hand-computed single-unit cases and the length
//! rules. Linux has no KUnit file for it.

use rinux_lib::crypto::nh::{NH_HASH_BYTES, nh, nh_bytes};

#[test]
fn zero_message_and_zero_key_hash_to_zero() {
    assert_eq!(nh(&[0u32; 16], &[0u8; 16]), [0; 4]);
}

#[test]
fn one_unit_multiplies_word_pairs_into_every_pass() {
    // Every pass multiplies the m0/m2 pair (and then the m1/m3 pair) with a
    // different key word, so with a zero key they all see the same products.
    let key = [0u32; 16];
    let mut msg = [0u8; 16];
    msg[0] = 1; // m0
    msg[8] = 1; // m2
    assert_eq!(nh(&key, &msg), [1, 1, 1, 1]);

    let mut msg = [0u8; 16];
    msg[4] = 2; // m1
    msg[12] = 3; // m3
    assert_eq!(nh(&key, &msg), [6, 6, 6, 6]);
}

#[test]
fn key_words_are_added_before_multiplying() {
    // (m0 + k0) * (m2 + k2) = (0 + 2) * (0 + 5) = 10 on pass 0.
    let mut key = [0u32; 16];
    key[0] = 2;
    key[2] = 5;
    assert_eq!(nh(&key, &[0u8; 16]), [10, 0, 0, 0]);
}

#[test]
fn second_unit_uses_the_key_advanced_by_four_words() {
    let mut key = [0u32; 20];
    key[4] = 3; // unit 1 reads key[4..20], so key[4] is its k0
    key[6] = 1; // and key[6] is its k2
    let mut msg = [0u8; 32];
    msg[16] = 1; // unit 1: m0 = 1
    msg[24] = 1; // unit 1: m2 = 1
    // Unit 1 pass 0: (1 + 3) * (1 + 1) = 8; unit 0 contributes nothing.
    assert_eq!(nh(&key, &msg)[0], 8);
}

#[test]
fn hash_bytes_are_little_endian() {
    let bytes = nh_bytes(&[0x0102, 0, 0, 0]);
    assert_eq!(bytes.len(), NH_HASH_BYTES);
    assert_eq!(&bytes[..2], &[0x02, 0x01]);
}

#[test]
#[should_panic]
fn a_short_key_is_refused() {
    let _ = nh(&[0u32; 4], &[0u8; 32]);
}

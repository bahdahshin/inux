// SPDX-License-Identifier: GPL-2.0-or-later
//! Host tests for the crypto XOR helpers and `crypto_memneq`. Linux has no
//! KUnit file for these generic routines.

use rinux_lib::crypto::memneq::crypto_memneq;
use rinux_lib::crypto::utils::{crypto_xor, crypto_xor_cpy};

#[test]
fn xor_cpy_is_bytewise_xor_at_every_length() {
    for len in 0..40 {
        let a: Vec<u8> = (0..len).map(|i| (i * 7 + 3) as u8).collect();
        let b: Vec<u8> = (0..len).map(|i| (i * 13 + 1) as u8).collect();
        let mut dst = vec![0u8; len];
        crypto_xor_cpy(&mut dst, &a, &b);
        let want: Vec<u8> = a.iter().zip(&b).map(|(x, y)| x ^ y).collect();
        assert_eq!(dst, want, "len {len}");
    }
}

#[test]
fn xor_in_place_is_its_own_inverse() {
    let original: Vec<u8> = (0..33).map(|i| i as u8 * 5).collect();
    let key: Vec<u8> = (0..33).map(|i| 0xa5 ^ i as u8).collect();
    let mut data = original.clone();
    crypto_xor(&mut data, &key);
    assert_ne!(data, original);
    crypto_xor(&mut data, &key);
    assert_eq!(data, original);
}

#[test]
fn memneq_detects_a_difference_at_any_position() {
    for len in 0..40 {
        let a: Vec<u8> = (0..len).map(|i| i as u8).collect();
        assert!(!crypto_memneq(&a, &a.clone()), "equal, len {len}");
        for pos in 0..len {
            let mut b = a.clone();
            b[pos] ^= 0x80;
            assert!(crypto_memneq(&a, &b), "len {len} pos {pos}");
        }
    }
}

#[test]
fn memneq_of_empty_strings_is_false() {
    assert!(!crypto_memneq(&[], &[]));
}

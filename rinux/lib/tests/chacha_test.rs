// SPDX-License-Identifier: GPL-2.0-or-later
//! Host tests for the generic ChaCha implementation, against the RFC 7539
//! test vectors. Linux has no KUnit file for the generic block function
//! beyond the crypto selftests.

use rinux_lib::crypto::chacha::{CHACHA_BLOCK_SIZE, ChachaState};

const KEY: [u8; 32] = [
    0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25,
    26, 27, 28, 29, 30, 31,
];

/// RFC 7539 section 2.3.2: counter 1, nonce 00:00:00:09:00:00:00:4a:00:00:00:00.
fn rfc_iv() -> [u8; 16] {
    let mut iv = [0u8; 16];
    iv[0..4].copy_from_slice(&1u32.to_le_bytes());
    iv[4..8].copy_from_slice(&[0x00, 0x00, 0x00, 0x09]);
    iv[8..12].copy_from_slice(&[0x00, 0x00, 0x00, 0x4a]);
    iv
}

#[test]
fn block_matches_the_rfc_7539_vector() {
    let mut state = ChachaState::new(&KEY, &rfc_iv());
    let mut out = [0u8; CHACHA_BLOCK_SIZE];
    state.chacha20_block(&mut out);
    assert_eq!(
        &out[..16],
        &[
            0x10, 0xf1, 0xe7, 0xe4, 0xd1, 0x3b, 0x59, 0x15, 0x50, 0x0f, 0xdd, 0x1f, 0xa3, 0x20,
            0x71, 0xc4
        ]
    );
    // The counter advanced to 2.
    assert_eq!(state.x[12], 2);
}

#[test]
fn encryption_matches_the_rfc_7539_sunscreen_vector() {
    let plaintext = b"Ladies and Gentlemen of the class of '99: If I could offer you only one tip for the future, sunscreen would be it.";
    let mut iv = [0u8; 16];
    iv[0..4].copy_from_slice(&1u32.to_le_bytes());
    iv[8..12].copy_from_slice(&[0x00, 0x00, 0x00, 0x4a]);
    let mut state = ChachaState::new(&KEY, &iv);
    let mut out = vec![0u8; plaintext.len()];
    state.chacha20_crypt(&mut out, plaintext);
    assert_eq!(&out[..8], &[0x6e, 0x2e, 0x35, 0x9a, 0x25, 0x68, 0xf9, 0x80]);
    // Decrypting from the same starting state gives the plaintext back.
    let mut again = ChachaState::new(&KEY, &iv);
    let mut back = vec![0u8; out.len()];
    again.chacha20_crypt(&mut back, &out);
    assert_eq!(back, plaintext);
}

#[test]
fn split_calls_match_one_call() {
    let data: Vec<u8> = (0..=255).cycle().take(300).collect();
    let mut whole = vec![0u8; data.len()];
    ChachaState::new(&KEY, &rfc_iv()).chacha20_crypt(&mut whole, &data);

    let mut state = ChachaState::new(&KEY, &rfc_iv());
    let mut parts = vec![0u8; data.len()];
    let (a, b) = parts.split_at_mut(64);
    state.chacha20_crypt(a, &data[..64]);
    state.chacha20_crypt(b, &data[64..]);
    assert_eq!(parts, whole);
}

#[test]
fn hchacha_outputs_eight_words_without_advancing() {
    let state = ChachaState::new(&KEY, &rfc_iv());
    let first = state.hchacha_block(20);
    assert_eq!(state.hchacha_block(20), first);
    assert_eq!(state.x[12], 1);
}

// SPDX-License-Identifier: GPL-2.0-or-later
//! Host tests for the ARC4 core. Linux has no KUnit file for it; these use
//! the published ARC4 test vectors (the keystream outputs widely used for
//! the cipher) and check that calls can be split.

use rinux_lib::crypto::arc4::Arc4Ctx;

fn encrypt(key: &[u8], data: &[u8]) -> Vec<u8> {
    let mut ctx = Arc4Ctx::new(key);
    let mut out = vec![0u8; data.len()];
    ctx.crypt(&mut out, data);
    out
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02X}")).collect()
}

#[test]
fn published_vectors() {
    assert_eq!(hex(&encrypt(b"Key", b"Plaintext")), "BBF316E8D940AF0AD3");
    assert_eq!(hex(&encrypt(b"Wiki", b"pedia")), "1021BF0420");
    assert_eq!(
        hex(&encrypt(b"Secret", b"Attack at dawn")),
        "45A01F645FC35B383552544B9BF5"
    );
}

#[test]
fn decryption_is_the_same_operation() {
    let cipher = encrypt(b"Key", b"Plaintext");
    assert_eq!(encrypt(b"Key", &cipher), b"Plaintext");
}

#[test]
fn split_calls_match_one_call() {
    let data: Vec<u8> = (0..=255).cycle().take(1000).collect();
    let whole = encrypt(b"some key", &data);

    let mut ctx = Arc4Ctx::new(b"some key");
    let mut parts = vec![0u8; data.len()];
    let (a, b) = parts.split_at_mut(333);
    ctx.crypt(a, &data[..333]);
    ctx.crypt(b, &data[333..]);
    assert_eq!(parts, whole);
}

#[test]
fn empty_input_leaves_the_state_alone() {
    let mut ctx = Arc4Ctx::new(b"k");
    let before = ctx;
    ctx.crypt(&mut [], &[]);
    assert_eq!(ctx, before);
}

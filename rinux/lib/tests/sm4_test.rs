// SPDX-License-Identifier: GPL-2.0-or-later
//! Host tests for the SM4 core, against the GB/T 32907 example and the
//! decryption round trip.

use rinux_lib::crypto::sm4::{sm4_crypt_block, sm4_expandkey};

const KEY: [u8; 16] = [
    0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef, 0xfe, 0xdc, 0xba, 0x98, 0x76, 0x54, 0x32, 0x10,
];

#[test]
fn standard_example_block() {
    let ctx = sm4_expandkey(&KEY);
    let ct = sm4_crypt_block(&ctx.rkey_enc, &KEY);
    let hex: String = ct.iter().map(|b| format!("{b:02x}")).collect();
    assert_eq!(hex, "681edf34d206965e86b3e94f536e4246");
}

#[test]
fn decryption_inverts_encryption() {
    let ctx = sm4_expandkey(&KEY);
    let block = *b"0123456789abcdef";
    let ct = sm4_crypt_block(&ctx.rkey_enc, &block);
    assert_eq!(sm4_crypt_block(&ctx.rkey_dec, &ct), block);
}

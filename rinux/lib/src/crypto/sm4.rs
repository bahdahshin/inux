// SPDX-License-Identifier: GPL-2.0-or-later
//! SM4 block cipher core (GB/T 32907-2016).
//!
//! Ported from `crypto/sm4.c`: `sm4_expandkey()` and `sm4_crypt_block()`, with
//! the constants from `crypto/sm4.c` and `include/crypto/sm4.h`. The Crypto API
//! registration in that file (`sm4_alg`) is not ported; this is the library
//! core it calls. The S-box is in `sm4_sbox.rs`, copied from the C table.

pub use super::sm4_sbox::SBOX;

/// `SM4_BLOCK_SIZE`.
pub const SM4_BLOCK_SIZE: usize = 16;
/// `SM4_KEY_SIZE`.
pub const SM4_KEY_SIZE: usize = 16;

/// `fk`: the system parameters that whiten the key.
const FK: [u32; 4] = [0xa3b1_bac6, 0x56aa_3350, 0x677d_9197, 0xb270_22dc];

/// `ck`: the round key constants.
const CK: [u32; 32] = [
    0x0007_0e15,
    0x1c23_2a31,
    0x383f_464d,
    0x545b_6269,
    0x7077_7e85,
    0x8c93_9aa1,
    0xa8af_b6bd,
    0xc4cb_d2d9,
    0xe0e7_eef5,
    0xfc03_0a11,
    0x181f_262d,
    0x343b_4249,
    0x5057_5e65,
    0x6c73_7a81,
    0x888f_969d,
    0xa4ab_b2b9,
    0xc0c7_ced5,
    0xdce3_eaf1,
    0xf8ff_060d,
    0x141b_2229,
    0x3037_3e45,
    0x4c53_5a61,
    0x686f_767d,
    0x848b_9299,
    0xa0a7_aeb5,
    0xbcc3_cad1,
    0xd8df_e6ed,
    0xf4fb_0209,
    0x1017_1e25,
    0x2c33_3a41,
    0x484f_565d,
    0x646b_7279,
];

/// `struct sm4_ctx`: the encryption and decryption round keys.
#[derive(Clone, Debug)]
pub struct Sm4Ctx {
    pub rkey_enc: [u32; 32],
    pub rkey_dec: [u32; 32],
}

fn t_non_lin_sub(x: u32) -> u32 {
    let b = x.to_le_bytes();
    u32::from_le_bytes([
        SBOX[usize::from(b[0])],
        SBOX[usize::from(b[1])],
        SBOX[usize::from(b[2])],
        SBOX[usize::from(b[3])],
    ])
}

fn key_lin_sub(x: u32) -> u32 {
    x ^ x.rotate_left(13) ^ x.rotate_left(23)
}

fn enc_lin_sub(x: u32) -> u32 {
    x ^ x.rotate_left(2) ^ x.rotate_left(10) ^ x.rotate_left(18) ^ x.rotate_left(24)
}

fn key_sub(x: u32) -> u32 {
    key_lin_sub(t_non_lin_sub(x))
}

fn enc_sub(x: u32) -> u32 {
    enc_lin_sub(t_non_lin_sub(x))
}

fn round(x0: u32, x1: u32, x2: u32, x3: u32, rk: u32) -> u32 {
    x0 ^ enc_sub(x1 ^ x2 ^ x3 ^ rk)
}

/// `sm4_expandkey()`: derive both round-key schedules from a 16-byte key.
#[allow(clippy::needless_range_loop)]
pub fn sm4_expandkey(key: &[u8; SM4_KEY_SIZE]) -> Sm4Ctx {
    let kw: [u32; 4] =
        core::array::from_fn(|i| u32::from_be_bytes(key[4 * i..4 * i + 4].try_into().unwrap()));
    let mut rk = [kw[0] ^ FK[0], kw[1] ^ FK[1], kw[2] ^ FK[2], kw[3] ^ FK[3]];
    let mut ctx = Sm4Ctx {
        rkey_enc: [0; 32],
        rkey_dec: [0; 32],
    };
    for i in (0..32).step_by(4) {
        rk[0] ^= key_sub(rk[1] ^ rk[2] ^ rk[3] ^ CK[i]);
        rk[1] ^= key_sub(rk[2] ^ rk[3] ^ rk[0] ^ CK[i + 1]);
        rk[2] ^= key_sub(rk[3] ^ rk[0] ^ rk[1] ^ CK[i + 2]);
        rk[3] ^= key_sub(rk[0] ^ rk[1] ^ rk[2] ^ CK[i + 3]);
        ctx.rkey_enc[i..i + 4].copy_from_slice(&rk);
        // Decryption uses the same keys in reverse order.
        for k in 0..4 {
            ctx.rkey_dec[31 - i - k] = rk[k];
        }
    }
    ctx
}

/// `sm4_crypt_block()`: run one block through the round keys `rk`
/// (`rkey_enc` to encrypt, `rkey_dec` to decrypt).
pub fn sm4_crypt_block(rk: &[u32; 32], input: &[u8; SM4_BLOCK_SIZE]) -> [u8; SM4_BLOCK_SIZE] {
    let mut x: [u32; 4] =
        core::array::from_fn(|i| u32::from_be_bytes(input[4 * i..4 * i + 4].try_into().unwrap()));
    for i in (0..32).step_by(4) {
        x[0] = round(x[0], x[1], x[2], x[3], rk[i]);
        x[1] = round(x[1], x[2], x[3], x[0], rk[i + 1]);
        x[2] = round(x[2], x[3], x[0], x[1], rk[i + 2]);
        x[3] = round(x[3], x[0], x[1], x[2], rk[i + 3]);
    }
    let mut out = [0u8; SM4_BLOCK_SIZE];
    for i in 0..4 {
        out[4 * i..4 * i + 4].copy_from_slice(&x[3 - i].to_be_bytes());
    }
    out
}

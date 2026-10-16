// SPDX-License-Identifier: GPL-2.0
//! The ChaCha stream cipher (RFC 7539), generic implementation.
//!
//! Ported from `lib/crypto/chacha.c`, `lib/crypto/chacha-block-generic.c` and
//! `include/crypto/chacha.h`. The state is sixteen 32-bit words: four
//! constants, eight key words, the 32-bit block counter in word 12 and the
//! 96-bit nonce in words 13 to 15. The architecture-specific SIMD versions
//! are not ported; they compute the same bytes.

use core::convert::TryInto;

/// `CHACHA_BLOCK_SIZE`.
pub const CHACHA_BLOCK_SIZE: usize = 64;
/// `CHACHA_KEY_SIZE`.
pub const CHACHA_KEY_SIZE: usize = 32;
/// `CHACHA_IV_SIZE`: the 32-bit counter followed by the 96-bit nonce.
pub const CHACHA_IV_SIZE: usize = 16;
/// `HCHACHA_OUT_WORDS`.
pub const HCHACHA_OUT_WORDS: usize = 8;

/// `struct chacha_state`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChachaState {
    /// The sixteen state words, in the order RFC 7539 lays them out.
    pub x: [u32; 16],
}

/// `chacha_permute()`: the rounds, `nrounds` of them (20 or 12).
fn chacha_permute(x: &mut [u32; 16], nrounds: u32) {
    // C whitelists these counts with a WARN_ON_ONCE; keep the same contract.
    debug_assert!(nrounds == 20 || nrounds == 12);

    // One double round is a column round then a diagonal round.
    fn qr(x: &mut [u32; 16], a: usize, b: usize, c: usize, d: usize) {
        x[a] = x[a].wrapping_add(x[b]);
        x[d] = (x[d] ^ x[a]).rotate_left(16);
        x[c] = x[c].wrapping_add(x[d]);
        x[b] = (x[b] ^ x[c]).rotate_left(12);
        x[a] = x[a].wrapping_add(x[b]);
        x[d] = (x[d] ^ x[a]).rotate_left(8);
        x[c] = x[c].wrapping_add(x[d]);
        x[b] = (x[b] ^ x[c]).rotate_left(7);
    }

    for _ in (0..nrounds).step_by(2) {
        qr(x, 0, 4, 8, 12);
        qr(x, 1, 5, 9, 13);
        qr(x, 2, 6, 10, 14);
        qr(x, 3, 7, 11, 15);
        qr(x, 0, 5, 10, 15);
        qr(x, 1, 6, 11, 12);
        qr(x, 2, 7, 8, 13);
        qr(x, 3, 4, 9, 14);
    }
}

impl ChachaState {
    /// `chacha_init()`: constants, the 256-bit key and the 128-bit IV.
    pub fn new(key: &[u8; CHACHA_KEY_SIZE], iv: &[u8; CHACHA_IV_SIZE]) -> Self {
        let mut x = [0u32; 16];
        x[0] = 0x6170_7865; // "expa"
        x[1] = 0x3320_646e; // "nd 3"
        x[2] = 0x7962_2d32; // "2-by"
        x[3] = 0x6b20_6574; // "te k"
        for (i, word) in key.chunks_exact(4).enumerate() {
            x[4 + i] = u32::from_le_bytes(word.try_into().unwrap());
        }
        for (i, word) in iv.chunks_exact(4).enumerate() {
            x[12 + i] = u32::from_le_bytes(word.try_into().unwrap());
        }
        Self { x }
    }

    /// `chacha_block_generic()`: the keystream block at the current counter,
    /// then the counter advances by one.
    pub fn block(&mut self, out: &mut [u8; CHACHA_BLOCK_SIZE], nrounds: u32) {
        let mut permuted = self.x;
        chacha_permute(&mut permuted, nrounds);
        for (i, chunk) in out.chunks_exact_mut(4).enumerate() {
            let word = permuted[i].wrapping_add(self.x[i]);
            chunk.copy_from_slice(&word.to_le_bytes());
        }
        self.x[12] = self.x[12].wrapping_add(1);
    }

    /// `chacha_crypt()`: XOR `src` with the keystream into `dst`, advancing
    /// the counter one block per 64 bytes. A short final block is applied
    /// partially, as in C.
    ///
    /// Panics if `dst` and `src` differ in length.
    pub fn crypt(&mut self, dst: &mut [u8], src: &[u8], nrounds: u32) {
        assert_eq!(
            dst.len(),
            src.len(),
            "ChaCha output and input lengths differ"
        );
        let mut stream = [0u8; CHACHA_BLOCK_SIZE];
        for (d, s) in dst
            .chunks_mut(CHACHA_BLOCK_SIZE)
            .zip(src.chunks(CHACHA_BLOCK_SIZE))
        {
            self.block(&mut stream, nrounds);
            for ((o, i), k) in d.iter_mut().zip(s).zip(stream.iter()) {
                *o = i ^ k;
            }
        }
    }

    /// `chacha20_block()`: a block with the 20-round ChaCha20.
    pub fn chacha20_block(&mut self, out: &mut [u8; CHACHA_BLOCK_SIZE]) {
        self.block(out, 20);
    }

    /// `chacha20_crypt()`: [`ChachaState::crypt`] with 20 rounds.
    pub fn chacha20_crypt(&mut self, dst: &mut [u8], src: &[u8]) {
        self.crypt(dst, src, 20);
    }

    /// `hchacha_block_generic()`: the HChaCha subkey derivation. Does not
    /// advance the counter.
    pub fn hchacha_block(&self, nrounds: u32) -> [u32; HCHACHA_OUT_WORDS] {
        let mut permuted = self.x;
        chacha_permute(&mut permuted, nrounds);
        let mut out = [0u32; HCHACHA_OUT_WORDS];
        out[..4].copy_from_slice(&permuted[..4]);
        out[4..].copy_from_slice(&permuted[12..16]);
        out
    }
}

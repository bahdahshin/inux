// SPDX-License-Identifier: GPL-2.0-only
//! The ARC4 stream cipher core.
//!
//! Ported from `lib/crypto/arc4.c` and `include/crypto/arc4.h`. The C code
//! keeps the 256-byte state `S` and two indices `x` and `y` in `struct
//! arc4_ctx`. ARC4 is obsolete and insecure; it is ported because the kernel
//! ships it as `ecb(arc4)`. The `crypto/arc4.c` algorithm wrapper (lskcipher
//! registration) belongs with the crypto API and is not ported here.
//!
//! The key-length limits are the header's `ARC4_MIN_KEY_SIZE` (1) and
//! `ARC4_MAX_KEY_SIZE` (256). C does not check them in `arc4_setkey()`; the
//! caller does, and here an empty key panics.

/// `ARC4_MIN_KEY_SIZE`.
pub const ARC4_MIN_KEY_SIZE: usize = 1;
/// `ARC4_MAX_KEY_SIZE`.
pub const ARC4_MAX_KEY_SIZE: usize = 256;
/// `ARC4_BLOCK_SIZE`: ARC4 is a stream cipher.
pub const ARC4_BLOCK_SIZE: usize = 1;

/// `struct arc4_ctx`: the permutation `S`, and the two stream indices.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Arc4Ctx {
    s: [u32; 256],
    x: u32,
    y: u32,
}

impl Arc4Ctx {
    /// `arc4_setkey()`: the key-scheduling step, which permutes `S` with the
    /// key bytes, repeating the key as needed.
    pub fn new(key: &[u8]) -> Self {
        assert!(
            (ARC4_MIN_KEY_SIZE..=ARC4_MAX_KEY_SIZE).contains(&key.len()),
            "ARC4 key length {} out of range",
            key.len()
        );
        let mut s = [0u32; 256];
        for (i, v) in s.iter_mut().enumerate() {
            *v = i as u32;
        }
        let mut j: u32 = 0;
        let mut k = 0;
        for i in 0..256 {
            let a = s[i];
            j = (j + u32::from(key[k]) + a) & 0xff;
            s[i] = s[j as usize];
            s[j as usize] = a;
            k += 1;
            if k >= key.len() {
                k = 0;
            }
        }
        Self { s, x: 1, y: 0 }
    }

    /// `arc4_crypt()`: XOR `input` with the keystream into `out`, advancing
    /// the state. Encryption and decryption are the same operation. Calls
    /// can be split at any point and give the same bytes as one call.
    ///
    /// Panics if `out` and `input` differ in length.
    pub fn crypt(&mut self, out: &mut [u8], input: &[u8]) {
        assert_eq!(
            out.len(),
            input.len(),
            "ARC4 output and input lengths differ"
        );
        if input.is_empty() {
            return;
        }
        let s = &mut self.s;
        let mut x = self.x;
        let mut y = self.y;
        let mut a = s[x as usize];
        y = (y + a) & 0xff;
        let mut b = s[y as usize];
        let last = input.len() - 1;
        for idx in 0..input.len() {
            s[y as usize] = a;
            a = (a + b) & 0xff;
            s[x as usize] = b;
            x = (x + 1) & 0xff;
            let ta = s[x as usize];
            let ty = (y + ta) & 0xff;
            let tb = s[ty as usize];
            out[idx] = input[idx] ^ s[a as usize] as u8;
            // As in C, the stored `y` is the one used for the last byte.
            if idx == last {
                break;
            }
            y = ty;
            a = ta;
            b = tb;
        }
        self.x = x;
        self.y = y;
    }
}

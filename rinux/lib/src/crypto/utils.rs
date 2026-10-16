// SPDX-License-Identifier: GPL-2.0-only
//! Byte-wise XOR helpers for the crypto code.
//!
//! Ported from `lib/crypto/utils.c` (`__crypto_xor()`) and the inline helpers
//! of `include/crypto/algapi.h` (`crypto_xor()` and `crypto_xor_cpy()`). The C
//! code moves whole words where alignment allows and bytes elsewhere. Those
//! word strides only change speed, so the result is the byte-wise XOR for every
//! input, and that is what is written here.

/// `crypto_xor_cpy()` (`__crypto_xor()`): `dst = src1 ^ src2` over `dst.len()`
/// bytes.
///
/// Panics if the three slices are not all the same length.
pub fn crypto_xor_cpy(dst: &mut [u8], src1: &[u8], src2: &[u8]) {
    assert!(dst.len() == src1.len() && src1.len() == src2.len());
    for ((d, a), b) in dst.iter_mut().zip(src1).zip(src2) {
        *d = a ^ b;
    }
}

/// `crypto_xor()`: `dst ^= src` over `dst.len()` bytes.
///
/// Panics if the slices differ in length.
pub fn crypto_xor(dst: &mut [u8], src: &[u8]) {
    assert_eq!(dst.len(), src.len());
    for (d, s) in dst.iter_mut().zip(src) {
        *d ^= s;
    }
}

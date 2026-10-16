// SPDX-License-Identifier: GPL-2.0
//! NH, an almost-universal hash over 32-bit words, used by Adiantum.
//!
//! Ported from `lib/crypto/nh.c` and `include/crypto/nh.h`. The message is
//! read in 16-byte units; each unit is added to the key word-wise, the sums
//! are multiplied in pairs, and four 64-bit accumulators collect the products
//! over all units. The architecture version in C returns `false` and the
//! generic loop runs; this is that loop.

/// `NH_MESSAGE_UNIT`: bytes consumed per step (two pairs of 32-bit words).
pub const NH_MESSAGE_UNIT: usize = 16;
/// `NH_NUM_PASSES`: accumulators in the output.
pub const NH_NUM_PASSES: usize = 4;
/// `NH_HASH_BYTES`: output size.
pub const NH_HASH_BYTES: usize = NH_NUM_PASSES * 8;

/// `nh()`: hash `message` (a multiple of 16 bytes) with `key`.
///
/// The key must have `4 * (units - 1) + 16` words, where `units` is
/// `message.len() / 16`, as C requires. Panics otherwise.
pub fn nh(key: &[u32], message: &[u8]) -> [u64; NH_NUM_PASSES] {
    assert_eq!(message.len() % NH_MESSAGE_UNIT, 0, "NH message length");
    let units = message.len() / NH_MESSAGE_UNIT;
    if units > 0 {
        assert!(key.len() >= 4 * (units - 1) + 16, "NH key too short");
    }
    let mut sums = [0u64; NH_NUM_PASSES];
    for (u, unit) in message.chunks_exact(NH_MESSAGE_UNIT).enumerate() {
        let k = &key[4 * u..];
        let w = |i: usize| u32::from_le_bytes(unit[4 * i..4 * i + 4].try_into().unwrap());
        let (m0, m1, m2, m3) = (w(0), w(1), w(2), w(3));
        // The C code adds the key word to the message word in 32 bits, then
        // multiplies the two sums as 64-bit products.
        sums[0] += u64::from(m0.wrapping_add(k[0])) * u64::from(m2.wrapping_add(k[2]));
        sums[1] += u64::from(m0.wrapping_add(k[4])) * u64::from(m2.wrapping_add(k[6]));
        sums[2] += u64::from(m0.wrapping_add(k[8])) * u64::from(m2.wrapping_add(k[10]));
        sums[3] += u64::from(m0.wrapping_add(k[12])) * u64::from(m2.wrapping_add(k[14]));
        sums[0] += u64::from(m1.wrapping_add(k[1])) * u64::from(m3.wrapping_add(k[3]));
        sums[1] += u64::from(m1.wrapping_add(k[5])) * u64::from(m3.wrapping_add(k[7]));
        sums[2] += u64::from(m1.wrapping_add(k[9])) * u64::from(m3.wrapping_add(k[11]));
        sums[3] += u64::from(m1.wrapping_add(k[13])) * u64::from(m3.wrapping_add(k[15]));
    }
    sums
}

/// The output as bytes, little-endian per accumulator (`cpu_to_le64`).
pub fn nh_bytes(hash: &[u64; NH_NUM_PASSES]) -> [u8; NH_HASH_BYTES] {
    let mut out = [0u8; NH_HASH_BYTES];
    for (chunk, v) in out.chunks_exact_mut(8).zip(hash) {
        chunk.copy_from_slice(&v.to_le_bytes());
    }
    out
}

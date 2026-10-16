// SPDX-License-Identifier: (GPL-2.0-only OR BSD-3-Clause)
//! SipHash: a fast short-input PRF.
//!
//! Ported from `lib/siphash.c` and `include/linux/siphash.h`.
//!
//! Copyright (C) 2016-2022 Jason A. Donenfeld <Jason@zx2c4.com>. All Rights
//! Reserved.
//!
//! <https://131002.net/siphash/>
//!
//! This is SipHash-2-4 for a secure PRF, and HalfSipHash-1-3 for an
//! insecure PRF only suitable for hash tables. As in Linux on 64-bit
//! machines, "HalfSipHash" is really SipHash-1-3 with its result truncated
//! to 32 bits.
//!
//! Reads are byte-wise little-endian, so the C aligned and unaligned entry
//! points are one function each here.

use crate::bitops::rol64;

/// `siphash_key_t`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SiphashKey {
    pub key: [u64; 2],
}

/// `hsiphash_key_t`: two `unsigned long`s.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HsiphashKey {
    pub key: [u64; 2],
}

/// `siphash_key_is_zero()`.
pub const fn siphash_key_is_zero(key: &SiphashKey) -> bool {
    key.key[0] | key.key[1] == 0
}

/// `SIPHASH_CONST_0`.
pub const SIPHASH_CONST_0: u64 = 0x736f6d6570736575;
/// `SIPHASH_CONST_1`.
pub const SIPHASH_CONST_1: u64 = 0x646f72616e646f6d;
/// `SIPHASH_CONST_2`.
pub const SIPHASH_CONST_2: u64 = 0x6c7967656e657261;
/// `SIPHASH_CONST_3`.
pub const SIPHASH_CONST_3: u64 = 0x7465646279746573;

/// The SipHash state: `v0..v3` and the final block `b`.
struct State {
    v: [u64; 4],
    b: u64,
}

impl State {
    /// `PREAMBLE(len)`.
    #[inline]
    fn new(len: usize, key: &[u64; 2]) -> State {
        State {
            v: [
                SIPHASH_CONST_0 ^ key[0],
                SIPHASH_CONST_1 ^ key[1],
                SIPHASH_CONST_2 ^ key[0],
                SIPHASH_CONST_3 ^ key[1],
            ],
            b: (len as u64) << 56,
        }
    }

    /// `SIPHASH_PERMUTATION()`.
    #[inline]
    fn round(&mut self) {
        let [a, b, c, d] = &mut self.v;
        *a = a.wrapping_add(*b);
        *b = rol64(*b, 13);
        *b ^= *a;
        *a = rol64(*a, 32);
        *c = c.wrapping_add(*d);
        *d = rol64(*d, 16);
        *d ^= *c;
        *a = a.wrapping_add(*d);
        *d = rol64(*d, 21);
        *d ^= *a;
        *c = c.wrapping_add(*b);
        *b = rol64(*b, 17);
        *b ^= *c;
        *c = rol64(*c, 32);
    }

    /// Absorbs one message word with `rounds` rounds.
    #[inline]
    fn compress(&mut self, m: u64, rounds: usize) {
        self.v[3] ^= m;
        for _ in 0..rounds {
            self.round();
        }
        self.v[0] ^= m;
    }

    /// `POSTAMBLE` (2 rounds per word, 4 final) or `HPOSTAMBLE` (1 and 3).
    #[inline]
    fn finish(mut self, rounds: usize, final_rounds: usize) -> u64 {
        let b = self.b;
        self.compress(b, rounds);
        self.v[2] ^= 0xff;
        for _ in 0..final_rounds {
            self.round();
        }
        (self.v[0] ^ self.v[1]) ^ (self.v[2] ^ self.v[3])
    }

    /// Absorbs `data`: whole little-endian words, then the tail into `b`.
    #[inline]
    fn absorb(&mut self, data: &[u8], rounds: usize) {
        let mut words = data.chunks_exact(8);
        for w in &mut words {
            self.compress(u64::from_le_bytes(w.try_into().unwrap()), rounds);
        }
        for (i, &byte) in words.remainder().iter().enumerate() {
            self.b |= (byte as u64) << (8 * i);
        }
    }
}

/// `siphash()`: SipHash-2-4 of `data`.
pub fn siphash(data: &[u8], key: &SiphashKey) -> u64 {
    let mut s = State::new(data.len(), &key.key);
    s.absorb(data, 2);
    s.finish(2, 4)
}

/// `siphash_1u64()`.
pub fn siphash_1u64(first: u64, key: &SiphashKey) -> u64 {
    let mut s = State::new(8, &key.key);
    s.compress(first, 2);
    s.finish(2, 4)
}

/// `siphash_2u64()`.
pub fn siphash_2u64(first: u64, second: u64, key: &SiphashKey) -> u64 {
    let mut s = State::new(16, &key.key);
    s.compress(first, 2);
    s.compress(second, 2);
    s.finish(2, 4)
}

/// `siphash_3u64()`.
pub fn siphash_3u64(first: u64, second: u64, third: u64, key: &SiphashKey) -> u64 {
    let mut s = State::new(24, &key.key);
    s.compress(first, 2);
    s.compress(second, 2);
    s.compress(third, 2);
    s.finish(2, 4)
}

/// `siphash_4u64()`.
pub fn siphash_4u64(first: u64, second: u64, third: u64, forth: u64, key: &SiphashKey) -> u64 {
    let mut s = State::new(32, &key.key);
    s.compress(first, 2);
    s.compress(second, 2);
    s.compress(third, 2);
    s.compress(forth, 2);
    s.finish(2, 4)
}

/// `siphash_1u32()`.
pub fn siphash_1u32(first: u32, key: &SiphashKey) -> u64 {
    let mut s = State::new(4, &key.key);
    s.b |= first as u64;
    s.finish(2, 4)
}

/// `siphash_2u32()`.
pub fn siphash_2u32(first: u32, second: u32, key: &SiphashKey) -> u64 {
    siphash_1u64(((second as u64) << 32) | first as u64, key)
}

/// `siphash_3u32()`.
pub fn siphash_3u32(first: u32, second: u32, third: u32, key: &SiphashKey) -> u64 {
    let combined = ((second as u64) << 32) | first as u64;
    let mut s = State::new(12, &key.key);
    s.compress(combined, 2);
    s.b |= third as u64;
    s.finish(2, 4)
}

/// `siphash_4u32()`.
pub fn siphash_4u32(first: u32, second: u32, third: u32, forth: u32, key: &SiphashKey) -> u64 {
    siphash_2u64(
        ((second as u64) << 32) | first as u64,
        ((forth as u64) << 32) | third as u64,
        key,
    )
}

/// `hsiphash()`: the 32-bit hash-table PRF of `data`.
pub fn hsiphash(data: &[u8], key: &HsiphashKey) -> u32 {
    let mut s = State::new(data.len(), &key.key);
    s.absorb(data, 1);
    s.finish(1, 3) as u32
}

/// `hsiphash_1u32()`.
pub fn hsiphash_1u32(first: u32, key: &HsiphashKey) -> u32 {
    let mut s = State::new(4, &key.key);
    s.b |= first as u64;
    s.finish(1, 3) as u32
}

/// `hsiphash_2u32()`.
pub fn hsiphash_2u32(first: u32, second: u32, key: &HsiphashKey) -> u32 {
    let mut s = State::new(8, &key.key);
    s.compress(((second as u64) << 32) | first as u64, 1);
    s.finish(1, 3) as u32
}

/// `hsiphash_3u32()`.
pub fn hsiphash_3u32(first: u32, second: u32, third: u32, key: &HsiphashKey) -> u32 {
    let mut s = State::new(12, &key.key);
    s.compress(((second as u64) << 32) | first as u64, 1);
    s.b |= third as u64;
    s.finish(1, 3) as u32
}

/// `hsiphash_4u32()`.
pub fn hsiphash_4u32(first: u32, second: u32, third: u32, forth: u32, key: &HsiphashKey) -> u32 {
    let mut s = State::new(16, &key.key);
    s.compress(((second as u64) << 32) | first as u64, 1);
    s.compress(((forth as u64) << 32) | third as u64, 1);
    s.finish(1, 3) as u32
}

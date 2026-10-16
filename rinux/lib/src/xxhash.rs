// SPDX-License-Identifier: GPL-2.0 OR BSD-2-Clause
//! xxHash: an extremely fast non-cryptographic hash.
//!
//! A port of `lib/xxhash.c` (Yann Collet's xxHash, as vendored in the Linux
//! tree): the one-shot [`xxh32`] and [`xxh64`] functions and the streaming
//! [`Xxh64State`] (`xxh64_reset`/`xxh64_update`/`xxh64_digest`). Used in the
//! kernel by zram/zswap and several filesystems.
//!
//! All arithmetic wraps, as the algorithm intends, and all input is read
//! little-endian (`get_unaligned_le*`). XXH32 has no streaming variant in the
//! kernel source, so none is ported.

// Round constants (the primes the algorithm mixes with).
const PRIME32_1: u32 = 2_654_435_761;
const PRIME32_2: u32 = 2_246_822_519;
const PRIME32_3: u32 = 3_266_489_917;
const PRIME32_4: u32 = 668_265_263;
const PRIME32_5: u32 = 374_761_393;

const PRIME64_1: u64 = 11_400_714_785_074_694_791;
const PRIME64_2: u64 = 14_029_467_366_897_019_727;
const PRIME64_3: u64 = 1_609_587_929_392_839_161;
const PRIME64_4: u64 = 9_650_029_242_287_828_579;
const PRIME64_5: u64 = 2_870_177_450_012_600_261;

#[inline]
fn le32(b: &[u8], i: usize) -> u32 {
    u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]])
}

#[inline]
fn le64(b: &[u8], i: usize) -> u64 {
    u64::from_le_bytes([
        b[i],
        b[i + 1],
        b[i + 2],
        b[i + 3],
        b[i + 4],
        b[i + 5],
        b[i + 6],
        b[i + 7],
    ])
}

#[inline]
fn xxh32_round(seed: u32, input: u32) -> u32 {
    let seed = seed.wrapping_add(input.wrapping_mul(PRIME32_2));
    seed.rotate_left(13).wrapping_mul(PRIME32_1)
}

/// `xxh32()`: the 32-bit xxHash of `input` with `seed`.
pub fn xxh32(input: &[u8], seed: u32) -> u32 {
    let len = input.len();
    let b_end = len;
    let mut p = 0usize;
    let mut h32;

    if len >= 16 {
        let limit = b_end - 16;
        let mut v1 = seed.wrapping_add(PRIME32_1).wrapping_add(PRIME32_2);
        let mut v2 = seed.wrapping_add(PRIME32_2);
        let mut v3 = seed;
        let mut v4 = seed.wrapping_sub(PRIME32_1);
        loop {
            v1 = xxh32_round(v1, le32(input, p));
            p += 4;
            v2 = xxh32_round(v2, le32(input, p));
            p += 4;
            v3 = xxh32_round(v3, le32(input, p));
            p += 4;
            v4 = xxh32_round(v4, le32(input, p));
            p += 4;
            if p > limit {
                break;
            }
        }
        h32 = v1
            .rotate_left(1)
            .wrapping_add(v2.rotate_left(7))
            .wrapping_add(v3.rotate_left(12))
            .wrapping_add(v4.rotate_left(18));
    } else {
        h32 = seed.wrapping_add(PRIME32_5);
    }

    h32 = h32.wrapping_add(len as u32);

    while p + 4 <= b_end {
        h32 = h32.wrapping_add(le32(input, p).wrapping_mul(PRIME32_3));
        h32 = h32.rotate_left(17).wrapping_mul(PRIME32_4);
        p += 4;
    }
    while p < b_end {
        h32 = h32.wrapping_add((input[p] as u32).wrapping_mul(PRIME32_5));
        h32 = h32.rotate_left(11).wrapping_mul(PRIME32_1);
        p += 1;
    }

    h32 ^= h32 >> 15;
    h32 = h32.wrapping_mul(PRIME32_2);
    h32 ^= h32 >> 13;
    h32 = h32.wrapping_mul(PRIME32_3);
    h32 ^= h32 >> 16;
    h32
}

#[inline]
fn xxh64_round(acc: u64, input: u64) -> u64 {
    let acc = acc.wrapping_add(input.wrapping_mul(PRIME64_2));
    acc.rotate_left(31).wrapping_mul(PRIME64_1)
}

#[inline]
fn xxh64_merge_round(acc: u64, val: u64) -> u64 {
    let val = xxh64_round(0, val);
    let acc = acc ^ val;
    acc.wrapping_mul(PRIME64_1).wrapping_add(PRIME64_4)
}

/// Shared tail + avalanche of the 64-bit hash: fold the remaining `tail`
/// bytes into `h64` and finalize.
fn xxh64_finalize(mut h64: u64, tail: &[u8]) -> u64 {
    let b_end = tail.len();
    let mut p = 0usize;
    while p + 8 <= b_end {
        let k1 = xxh64_round(0, le64(tail, p));
        h64 ^= k1;
        h64 = h64
            .rotate_left(27)
            .wrapping_mul(PRIME64_1)
            .wrapping_add(PRIME64_4);
        p += 8;
    }
    if p + 4 <= b_end {
        h64 ^= (le32(tail, p) as u64).wrapping_mul(PRIME64_1);
        h64 = h64
            .rotate_left(23)
            .wrapping_mul(PRIME64_2)
            .wrapping_add(PRIME64_3);
        p += 4;
    }
    while p < b_end {
        h64 ^= (tail[p] as u64).wrapping_mul(PRIME64_5);
        h64 = h64.rotate_left(11).wrapping_mul(PRIME64_1);
        p += 1;
    }

    h64 ^= h64 >> 33;
    h64 = h64.wrapping_mul(PRIME64_2);
    h64 ^= h64 >> 29;
    h64 = h64.wrapping_mul(PRIME64_3);
    h64 ^= h64 >> 32;
    h64
}

/// `xxh64()`: the 64-bit xxHash of `input` with `seed`.
pub fn xxh64(input: &[u8], seed: u64) -> u64 {
    let len = input.len();
    let b_end = len;
    let mut p = 0usize;
    let mut h64;

    if len >= 32 {
        let limit = b_end - 32;
        let mut v1 = seed.wrapping_add(PRIME64_1).wrapping_add(PRIME64_2);
        let mut v2 = seed.wrapping_add(PRIME64_2);
        let mut v3 = seed;
        let mut v4 = seed.wrapping_sub(PRIME64_1);
        loop {
            v1 = xxh64_round(v1, le64(input, p));
            p += 8;
            v2 = xxh64_round(v2, le64(input, p));
            p += 8;
            v3 = xxh64_round(v3, le64(input, p));
            p += 8;
            v4 = xxh64_round(v4, le64(input, p));
            p += 8;
            if p > limit {
                break;
            }
        }
        h64 = v1
            .rotate_left(1)
            .wrapping_add(v2.rotate_left(7))
            .wrapping_add(v3.rotate_left(12))
            .wrapping_add(v4.rotate_left(18));
        h64 = xxh64_merge_round(h64, v1);
        h64 = xxh64_merge_round(h64, v2);
        h64 = xxh64_merge_round(h64, v3);
        h64 = xxh64_merge_round(h64, v4);
    } else {
        h64 = seed.wrapping_add(PRIME64_5);
    }

    h64 = h64.wrapping_add(len as u64);
    xxh64_finalize(h64, &input[p..])
}

/// `struct xxh64_state`: the running state of a streaming 64-bit hash.
pub struct Xxh64State {
    total_len: u64,
    v1: u64,
    v2: u64,
    v3: u64,
    v4: u64,
    /// Up to one unprocessed 32-byte block (`mem64` in the C struct).
    mem: [u8; 32],
    memsize: usize,
}

impl Xxh64State {
    /// `xxh64_reset()`: a fresh state seeded with `seed`.
    pub fn new(seed: u64) -> Xxh64State {
        Xxh64State {
            total_len: 0,
            v1: seed.wrapping_add(PRIME64_1).wrapping_add(PRIME64_2),
            v2: seed.wrapping_add(PRIME64_2),
            v3: seed,
            v4: seed.wrapping_sub(PRIME64_1),
            mem: [0; 32],
            memsize: 0,
        }
    }

    /// `xxh64_update()`: fold `input` into the state.
    pub fn update(&mut self, input: &[u8]) {
        let len = input.len();
        let b_end = len;
        let mut p = 0usize;
        self.total_len = self.total_len.wrapping_add(len as u64);

        if self.memsize + len < 32 {
            self.mem[self.memsize..self.memsize + len].copy_from_slice(input);
            self.memsize += len;
            return;
        }

        if self.memsize != 0 {
            // Complete the buffered block with the head of this input.
            let need = 32 - self.memsize;
            self.mem[self.memsize..32].copy_from_slice(&input[..need]);
            self.v1 = xxh64_round(self.v1, le64(&self.mem, 0));
            self.v2 = xxh64_round(self.v2, le64(&self.mem, 8));
            self.v3 = xxh64_round(self.v3, le64(&self.mem, 16));
            self.v4 = xxh64_round(self.v4, le64(&self.mem, 24));
            p += need;
            self.memsize = 0;
        }

        if p + 32 <= b_end {
            let limit = b_end - 32;
            let mut v1 = self.v1;
            let mut v2 = self.v2;
            let mut v3 = self.v3;
            let mut v4 = self.v4;
            loop {
                v1 = xxh64_round(v1, le64(input, p));
                p += 8;
                v2 = xxh64_round(v2, le64(input, p));
                p += 8;
                v3 = xxh64_round(v3, le64(input, p));
                p += 8;
                v4 = xxh64_round(v4, le64(input, p));
                p += 8;
                if p > limit {
                    break;
                }
            }
            self.v1 = v1;
            self.v2 = v2;
            self.v3 = v3;
            self.v4 = v4;
        }

        if p < b_end {
            let rem = b_end - p;
            self.mem[..rem].copy_from_slice(&input[p..b_end]);
            self.memsize = rem;
        }
    }

    /// `xxh64_digest()`: the hash of everything fed in so far.
    pub fn digest(&self) -> u64 {
        let mut h64;
        if self.total_len >= 32 {
            h64 = self
                .v1
                .rotate_left(1)
                .wrapping_add(self.v2.rotate_left(7))
                .wrapping_add(self.v3.rotate_left(12))
                .wrapping_add(self.v4.rotate_left(18));
            h64 = xxh64_merge_round(h64, self.v1);
            h64 = xxh64_merge_round(h64, self.v2);
            h64 = xxh64_merge_round(h64, self.v3);
            h64 = xxh64_merge_round(h64, self.v4);
        } else {
            h64 = self.v3.wrapping_add(PRIME64_5);
        }
        h64 = h64.wrapping_add(self.total_len);
        xxh64_finalize(h64, &self.mem[..self.memsize])
    }
}

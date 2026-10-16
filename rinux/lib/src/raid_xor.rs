// SPDX-License-Identifier: GPL-2.0-or-later
//! The generic 8-register XOR for RAID: `dst ^= src1 ^ src2 ...` over
//! machine words, eight words per step.
//!
//! Ported from `lib/raid/xor/xor-8regs.c`. The C unrolls by eight words and
//! processes `bytes / sizeof(long) / 8` such groups, so a trailing partial
//! group is not touched. [`chunks_exact`](slice::chunks_exact) gives the same
//! groups here. The C `DO_XOR_BLOCKS` macro becomes the four functions below,
//! one per source count, which the template in `xor_impl.h` generated.

/// `xor_8regs_2()`: `p1 ^= p2` over whole eight-word groups.
pub fn xor_2(p1: &mut [u64], p2: &[u64]) {
    for (a, b) in p1.chunks_exact_mut(8).zip(p2.chunks_exact(8)) {
        for k in 0..8 {
            a[k] ^= b[k];
        }
    }
}

/// `xor_8regs_3()`: `p1 ^= p2 ^ p3` over whole eight-word groups.
pub fn xor_3(p1: &mut [u64], p2: &[u64], p3: &[u64]) {
    for ((a, b), c) in p1
        .chunks_exact_mut(8)
        .zip(p2.chunks_exact(8))
        .zip(p3.chunks_exact(8))
    {
        for k in 0..8 {
            a[k] ^= b[k] ^ c[k];
        }
    }
}

/// `xor_8regs_4()`: `p1 ^= p2 ^ p3 ^ p4` over whole eight-word groups.
pub fn xor_4(p1: &mut [u64], p2: &[u64], p3: &[u64], p4: &[u64]) {
    for (((a, b), c), d) in p1
        .chunks_exact_mut(8)
        .zip(p2.chunks_exact(8))
        .zip(p3.chunks_exact(8))
        .zip(p4.chunks_exact(8))
    {
        for k in 0..8 {
            a[k] ^= b[k] ^ c[k] ^ d[k];
        }
    }
}

/// `xor_8regs_5()`: `p1 ^= p2 ^ p3 ^ p4 ^ p5` over whole eight-word groups.
pub fn xor_5(p1: &mut [u64], p2: &[u64], p3: &[u64], p4: &[u64], p5: &[u64]) {
    for ((((a, b), c), d), e) in p1
        .chunks_exact_mut(8)
        .zip(p2.chunks_exact(8))
        .zip(p3.chunks_exact(8))
        .zip(p4.chunks_exact(8))
        .zip(p5.chunks_exact(8))
    {
        for k in 0..8 {
            a[k] ^= b[k] ^ c[k] ^ d[k] ^ e[k];
        }
    }
}

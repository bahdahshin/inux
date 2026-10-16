// SPDX-License-Identifier: GPL-2.0
//! Hamming weight: the number of set bits in a word.
//!
//! Ported from `lib/hweight.c`, using the `CONFIG_ARCH_HAS_FAST_MULTIPLIER`
//! variants that x86 selects. `count_ones()` gives the same answers; these
//! exist so `__sw_hweight*()` callers port directly.

/// `__sw_hweight32()`.
pub const fn sw_hweight32(mut w: u32) -> u32 {
    w -= (w >> 1) & 0x5555_5555;
    w = (w & 0x3333_3333) + ((w >> 2) & 0x3333_3333);
    w = (w + (w >> 4)) & 0x0f0f_0f0f;
    w.wrapping_mul(0x0101_0101) >> 24
}

/// `__sw_hweight16()`.
pub const fn sw_hweight16(w: u32) -> u32 {
    let mut res = w - ((w >> 1) & 0x5555);
    res = (res & 0x3333) + ((res >> 2) & 0x3333);
    res = (res + (res >> 4)) & 0x0f0f;
    (res + (res >> 8)) & 0x00ff
}

/// `__sw_hweight8()`.
pub const fn sw_hweight8(w: u32) -> u32 {
    let mut res = w - ((w >> 1) & 0x55);
    res = (res & 0x33) + ((res >> 2) & 0x33);
    (res + (res >> 4)) & 0x0f
}

/// `__sw_hweight64()`.
pub const fn sw_hweight64(mut w: u64) -> u64 {
    w -= (w >> 1) & 0x5555_5555_5555_5555;
    w = (w & 0x3333_3333_3333_3333) + ((w >> 2) & 0x3333_3333_3333_3333);
    w = (w + (w >> 4)) & 0x0f0f_0f0f_0f0f_0f0f;
    w.wrapping_mul(0x0101_0101_0101_0101) >> 56
}

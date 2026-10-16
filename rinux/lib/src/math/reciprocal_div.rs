// SPDX-License-Identifier: GPL-2.0
//! Division by an invariant divisor using multiplication.
//!
//! Ported from `lib/math/reciprocal_div.c` and `include/linux/reciprocal_div.h`.
//!
//! Based on "Division by Invariant Integers Using Multiplication" by
//! Torbjörn Granlund and Peter L. Montgomery, via Agner Fog's assembler
//! implementation. Computing the reciprocal is slow; dividing by it is a
//! multiply and two shifts.

use super::fls;

/// Precomputed reciprocal for [`reciprocal_divide`] (Figure 4.1 of the paper).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct ReciprocalValue {
    pub m: u32,
    pub sh1: u8,
    pub sh2: u8,
}

/// `reciprocal_value()`: precomputes the reciprocal of `d`, which must be
/// non-zero.
pub fn reciprocal_value(d: u32) -> ReciprocalValue {
    let l = fls(d.wrapping_sub(1));
    let mut m = (1u64 << 32) * ((1u64 << l) - d as u64);
    m /= d as u64;
    m += 1;
    ReciprocalValue {
        m: m as u32,
        sh1: l.min(1) as u8,
        sh2: l.saturating_sub(1) as u8,
    }
}

/// `reciprocal_divide()`: `a / d`, where `r` is `reciprocal_value(d)`.
#[inline]
pub fn reciprocal_divide(a: u32, r: ReciprocalValue) -> u32 {
    let t = ((a as u64 * r.m as u64) >> 32) as u32;
    (t + ((a - t) >> r.sh1)) >> r.sh2
}

/// Result of [`reciprocal_value_adv`] (Figure 4.2 of the paper).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct ReciprocalValueAdv {
    pub m: u32,
    pub sh: u8,
    pub exp: u8,
    pub is_wide_m: bool,
}

/// `reciprocal_value_adv()`: the advanced reciprocal of `d` with `prec` bits
/// of precision, used for JIT divide code generation.
///
/// Divisors above `1 << 31` are not supported and warn; callers handle them
/// before calling, as the C header describes.
pub fn reciprocal_value_adv(d: u32, prec: u8) -> ReciprocalValueAdv {
    // ceil(log2(d))
    let l = fls(d.wrapping_sub(1));
    crate::warn_on!(l == 32);

    // The shifts wrap like x86 shift instructions when l == 32.
    let mut post_shift = l;
    let mut mlow = 1u64.wrapping_shl(32 + l) / d as u64;
    let mut mhigh = 1u64
        .wrapping_shl(32 + l)
        .wrapping_add(1u64.wrapping_shl((32 + l).wrapping_sub(prec as u32)))
        / d as u64;

    while post_shift > 0 {
        let lo = mlow >> 1;
        let hi = mhigh >> 1;
        if lo >= hi {
            break;
        }
        mlow = lo;
        mhigh = hi;
        post_shift -= 1;
    }

    ReciprocalValueAdv {
        m: mhigh as u32,
        sh: post_shift as u8,
        exp: l as u8,
        is_wide_m: mhigh > u32::MAX as u64,
    }
}

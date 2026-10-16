// SPDX-License-Identifier: GPL-2.0-only
//! Lowest common multiple.
//!
//! Ported from `lib/math/lcm.c`.

use super::gcd;

/// `lcm()`: lowest common multiple, or 0 if either argument is 0.
///
/// Like C, the multiplication wraps if the result does not fit.
pub fn lcm(a: u64, b: u64) -> u64 {
    if a != 0 && b != 0 {
        (a / gcd(a, b)).wrapping_mul(b)
    } else {
        0
    }
}

/// `lcm_not_zero()`: [`lcm`], or whichever argument is non-zero.
pub fn lcm_not_zero(a: u64, b: u64) -> u64 {
    match lcm(a, b) {
        0 if b != 0 => b,
        0 => a,
        l => l,
    }
}

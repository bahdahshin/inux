// SPDX-License-Identifier: GPL-2.0
//! Integer square root.
//!
//! Ported from `lib/math/int_sqrt.c`.
//!
//! Copyright (C) 2013 Davidlohr Bueso <davidlohr.bueso@hp.com>
//!
//! Based on the shift-and-subtract algorithm for computing integer square
//! root from Guy L. Steele.

use super::fls0;

/// `int_sqrt()`: `floor(sqrt(x))`.
pub fn int_sqrt(mut x: u64) -> u64 {
    if x <= 1 {
        return x;
    }

    let mut y = 0;
    let mut m = 1u64 << (fls0(x) & !1);
    while m != 0 {
        let b = y + m;
        y >>= 1;

        if x >= b {
            x -= b;
            y += m;
        }
        m >>= 2;
    }
    y
}

/// `int_sqrt64()`: `floor(sqrt(x))` as a `u32`.
///
/// On 64-bit kernels this is [`int_sqrt`] with the result narrowed.
pub fn int_sqrt64(x: u64) -> u32 {
    int_sqrt(x) as u32
}

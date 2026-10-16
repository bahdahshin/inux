// SPDX-License-Identifier: GPL-2.0
//! Integer power.
//!
//! Ported from `lib/math/int_pow.c`, itself derived from
//! `drivers/video/backlight/pwm_bl.c`.

/// `int_pow()`: `base` raised to `exp`. Wraps on overflow like the C code.
pub const fn int_pow(mut base: u64, mut exp: u32) -> u64 {
    let mut result: u64 = 1;
    while exp != 0 {
        if exp & 1 != 0 {
            result = result.wrapping_mul(base);
        }
        exp >>= 1;
        base = base.wrapping_mul(base);
    }
    result
}

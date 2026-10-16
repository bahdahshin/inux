// SPDX-License-Identifier: GPL-2.0
//! Integer math helpers.
//!
//! Ported from `lib/math/` and the 64-bit parts of `include/linux/math64.h`.
//!
//! rinux targets LP64 machines, so C `unsigned long` maps to `u64` and
//! `long` to `i64` throughout.

mod cordic;
mod div64;
mod gcd;
mod int_log;
mod int_pow;
mod int_sqrt;
mod lcm;
mod polynomial;
mod rational;
mod reciprocal_div;

pub use cordic::{CordicIq, cordic_calc_iq};
pub use div64::*;
pub use gcd::{EFFICIENT_FFS, gcd};
pub use int_log::{intlog2, intlog10};
pub use int_pow::int_pow;
pub use int_sqrt::{int_sqrt, int_sqrt64};
pub use lcm::{lcm, lcm_not_zero};
pub use polynomial::{Polynomial, PolynomialTerm, polynomial_calc};
pub use rational::rational_best_approximation;
pub use reciprocal_div::{
    ReciprocalValue, ReciprocalValueAdv, reciprocal_divide, reciprocal_value, reciprocal_value_adv,
};

/// `__ffs()`: index of the lowest set bit. Undefined for 0 in C; panics here
/// in debug builds.
#[inline]
pub(crate) const fn ffs0(x: u64) -> u32 {
    debug_assert!(x != 0);
    x.trailing_zeros()
}

/// `fls()`: 1-based index of the highest set bit, 0 for 0.
#[inline]
pub const fn fls(x: u32) -> u32 {
    32 - x.leading_zeros()
}

/// `fls64()`: 1-based index of the highest set bit, 0 for 0.
#[inline]
pub const fn fls64(x: u64) -> u32 {
    64 - x.leading_zeros()
}

/// `__fls()`: 0-based index of the highest set bit. Undefined for 0.
#[inline]
pub(crate) const fn fls0(x: u64) -> u32 {
    debug_assert!(x != 0);
    63 - x.leading_zeros()
}

// SPDX-License-Identifier: GPL-2.0-only
//! Polynomial evaluation with integer coefficients.
//!
//! Ported from `lib/math/polynomial.c` and `include/linux/polynomial.h`.
//! Drivers that convert sensor readings use these to avoid floating point:
//! each coefficient is a fraction, and the factors are redistributed so the
//! intermediate values stay within `long`.
//!
//! C's flexible array member `terms[]` becomes a slice. The C loop walks the
//! terms until one with `deg == 0`, the constant term, which is the last
//! term; [`polynomial_calc`] does the same and stops at the slice's end if
//! no constant term is present, where C would read past the array.

/// `struct polynomial_term`: `coef * data^deg / divider / divider_leftover`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PolynomialTerm {
    /// Power of the input for this term.
    pub deg: u32,
    /// Numerator of the coefficient.
    pub coef: i64,
    /// Divider applied with each power of the input.
    pub divider: i64,
    /// Divider applied once to the whole term.
    pub divider_leftover: i64,
}

/// `struct polynomial`: a list of terms, highest degree first, and the
/// divider shared by the whole sum.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Polynomial<'a> {
    /// Divider applied to the sum; zero means one, as in C's `?:`.
    pub total_divider: i64,
    /// The terms, ending with the one whose `deg` is zero.
    pub terms: &'a [PolynomialTerm],
}

/// `mult_frac()` from `include/linux/math.h`: `x * numer / denom` without
/// overflowing the intermediate product of the high part.
fn mult_frac(x: i64, numer: i64, denom: i64) -> i64 {
    let quot = x / denom;
    let rem = x % denom;
    quot.wrapping_mul(numer)
        .wrapping_add(rem.wrapping_mul(numer) / denom)
}

/// `polynomial_calc()`: evaluate `poly` at `data`.
///
/// C overflow is undefined; the wrapping arithmetic here matches what the
/// kernel's `-fno-strict-overflow` build produces on x86-64.
pub fn polynomial_calc(poly: &Polynomial<'_>, data: i64) -> i64 {
    let total_divider = if poly.total_divider == 0 {
        1
    } else {
        poly.total_divider
    };
    let mut ret: i64 = 0;

    for term in poly.terms {
        let mut tmp = term.coef;
        for _ in 0..term.deg {
            tmp = mult_frac(tmp, data, term.divider);
        }
        ret = ret.wrapping_add(tmp / term.divider_leftover);
        if term.deg == 0 {
            break;
        }
    }

    ret / total_divider
}

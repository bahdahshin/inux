// SPDX-License-Identifier: GPL-2.0
//! Rational approximation.
//!
//! Ported from `lib/math/rational.c`.
//!
//! Copyright (C) 2009 emlix GmbH, Oskar Schirmer <oskar@scara.com>
//! Copyright (C) 2019 Trent Piepho <tpiepho@gmail.com>

/// `rational_best_approximation()`: the closest fraction to
/// `given_numerator / given_denominator` whose parts fit the given maxima.
///
/// Useful for register fields: to program a PLL with a 5-bit denominator
/// and 8-bit numerator to a ratio of 3.1415, call
/// `rational_best_approximation(31415, 10000, (1 << 8) - 1, (1 << 5) - 1)`.
///
/// Works through the continued fraction of the input
/// (<https://en.wikipedia.org/wiki/Continued_fraction>). Returns
/// `(numerator, denominator)`. Arithmetic wraps like C `unsigned long`.
pub fn rational_best_approximation(
    given_numerator: u64,
    given_denominator: u64,
    max_numerator: u64,
    max_denominator: u64,
) -> (u64, u64) {
    // n/d is the remaining fraction, reduced each iteration by the
    // Euclidean algorithm; dp is d from the prior iteration. n2/d2, n1/d1
    // and n0/d0 are the current, previous and two-prior convergents, and a
    // is the current term of the continued fraction.
    let mut n = given_numerator;
    let mut d = given_denominator;
    let (mut n0, mut d1) = (0u64, 0u64);
    let (mut n1, mut d0) = (1u64, 1u64);

    loop {
        if d == 0 {
            break;
        }
        // Next term of the continued fraction.
        let dp = d;
        let a = n / d;
        d = n % d;
        n = dp;

        // The current convergent.
        let n2 = n0.wrapping_add(a.wrapping_mul(n1));
        let d2 = d0.wrapping_add(a.wrapping_mul(d1));

        // If it exceeds the maxima, return the previous convergent or the
        // largest semi-convergent, whichever is closer.
        if n2 > max_numerator || d2 > max_denominator {
            let mut t = u64::MAX;
            if let Some(q) = max_denominator.wrapping_sub(d0).checked_div(d1) {
                t = q;
            }
            if let Some(q) = max_numerator.wrapping_sub(n0).checked_div(n1) {
                t = t.min(q);
            }

            // With d1 == 0 there is no previous convergent, so always take
            // the semi-convergent.
            if d1 == 0
                || t.wrapping_mul(2) > a
                || (t.wrapping_mul(2) == a && d0.wrapping_mul(dp) > d1.wrapping_mul(d))
            {
                n1 = n0.wrapping_add(t.wrapping_mul(n1));
                d1 = d0.wrapping_add(t.wrapping_mul(d1));
            }
            break;
        }
        n0 = n1;
        n1 = n2;
        d0 = d1;
        d1 = d2;
    }
    (n1, d1)
}

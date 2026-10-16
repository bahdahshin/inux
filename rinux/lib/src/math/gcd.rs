// SPDX-License-Identifier: GPL-2.0-only
//! Greatest common divisor.
//!
//! Ported from `lib/math/gcd.c`. This is the binary GCD algorithm (often
//! attributed to Stein, though Knuth notes it appears in a first-century
//! Chinese math text). It beats the division-based algorithm even on x86.

use core::mem::swap;
use core::sync::atomic::{AtomicBool, Ordering};

use super::ffs0;

/// `efficient_ffs_key`: true when the CPU has a fast find-first-set.
///
/// With a fast `__ffs()` the shift-based loop wins; without one the
/// even/odd loop does. Architecture setup may clear this.
pub static EFFICIENT_FFS: AtomicBool = AtomicBool::new(true);

fn binary_gcd(mut a: u64, mut b: u64) -> u64 {
    let r = a | b;

    b >>= ffs0(b);
    if b == 1 {
        return r & r.wrapping_neg();
    }

    loop {
        a >>= ffs0(a);
        if a == 1 {
            return r & r.wrapping_neg();
        }
        if a == b {
            return a << ffs0(r);
        }
        if a < b {
            swap(&mut a, &mut b);
        }
        a -= b;
    }
}

fn even_odd_gcd(mut a: u64, mut b: u64) -> u64 {
    // Isolate the lowest set bit of a | b.
    let mut r = a | b;
    r &= r.wrapping_neg();

    while b & r == 0 {
        b >>= 1;
    }
    if b == r {
        return r;
    }

    loop {
        while a & r == 0 {
            a >>= 1;
        }
        if a == r {
            return r;
        }
        if a == b {
            return a;
        }
        if a < b {
            swap(&mut a, &mut b);
        }
        a -= b;
        a >>= 1;
        if a & r != 0 {
            a += b;
        }
        a >>= 1;
    }
}

/// `gcd()`: greatest common divisor of `a` and `b`. `gcd(0, b)` is `b`.
pub fn gcd(a: u64, b: u64) -> u64 {
    if a == 0 || b == 0 {
        return a | b;
    }
    if EFFICIENT_FFS.load(Ordering::Relaxed) {
        binary_gcd(a, b)
    } else {
        even_odd_gcd(a, b)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_algorithms_agree() {
        let mut x: u64 = 0x9e37_79b9_7f4a_7c15;
        for _ in 0..20_000 {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            let a = x >> (x % 50);
            let b = x.rotate_left(29) >> (x % 37);
            if a == 0 || b == 0 {
                continue;
            }
            assert_eq!(binary_gcd(a, b), even_odd_gcd(a, b), "gcd({a}, {b})");
        }
    }
}

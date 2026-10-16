// SPDX-License-Identifier: GPL-2.0-only
//! Prime numbers: a sieve over a caller-owned bitmap, with a trial-division
//! fallback past it.
//!
//! Ported from `lib/math/prime_numbers.c`. The C file grows its sieve on demand
//! with `kmalloc()` and RCU, and keeps a small static table of the first
//! primes. Here the sieve is built once over the bitmap the caller provides,
//! and numbers past its end use the same trial division as C
//! (`slow_is_prime_number()` and `slow_next_prime_number()`). The growth, RCU
//! and mutex are not ported: they exist so the table can be enlarged while
//! other CPUs read it.

/// The sieve: bit `n` of `bits` is set when `n` is prime, for `n < sz`.
#[derive(Debug)]
pub struct Primes<'a> {
    bits: &'a mut [u64],
    sz: usize,
}

fn test_bit(bits: &[u64], n: usize) -> bool {
    bits[n / 64] & (1 << (n % 64)) != 0
}

fn clear_bit(bits: &mut [u64], n: usize) {
    bits[n / 64] &= !(1 << (n % 64));
}

/// `slow_is_prime_number()`: trial division.
pub fn slow_is_prime_number(x: u64) -> bool {
    let mut y = isqrt(x);
    while y > 1 {
        if x % y == 0 {
            break;
        }
        y -= 1;
    }
    y == 1
}

/// `slow_next_prime_number()`: the next prime after `x`, by trial division.
pub fn slow_next_prime_number(mut x: u64) -> u64 {
    while x < u64::MAX {
        x += 1;
        if slow_is_prime_number(x) {
            break;
        }
    }
    x
}

/// Integer square root: the largest `r` with `r * r <= x`. `no_std` has no
/// float `sqrt`, so this uses Newton's method on integers.
fn isqrt(x: u64) -> u64 {
    if x < 2 {
        return x;
    }
    let mut r = x;
    let mut y = (r + 1) / 2;
    while y < r {
        r = y;
        y = (r + x / r) / 2;
    }
    r
}

impl<'a> Primes<'a> {
    /// Build the sieve over `0..bits.len() * 64` by the Sieve of
    /// Eratosthenes. Panics if `bits` is empty.
    pub fn new(bits: &'a mut [u64]) -> Self {
        assert!(!bits.is_empty(), "prime sieve needs storage");
        let sz = bits.len() * 64;
        bits.fill(u64::MAX);
        clear_bit(bits, 0);
        clear_bit(bits, 1);
        for y in 2..sz {
            if test_bit(bits, y) {
                let mut m = 2 * y;
                while m < sz {
                    clear_bit(bits, m);
                    m += y;
                }
            }
        }
        Self { bits, sz }
    }

    /// `is_prime_number()`: sieve lookup inside the table, trial division past it.
    pub fn is_prime(&self, x: u64) -> bool {
        match usize::try_from(x) {
            Ok(n) if n < self.sz => test_bit(self.bits, n),
            _ => slow_is_prime_number(x),
        }
    }

    /// `next_prime_number()`: the smallest prime greater than `x`.
    pub fn next_prime(&self, x: u64) -> u64 {
        let start = match usize::try_from(x) {
            Ok(n) if n + 1 < self.sz => n + 1,
            _ => return slow_next_prime_number(x),
        };
        (start..self.sz)
            .find(|&n| test_bit(self.bits, n))
            .map_or_else(
                || slow_next_prime_number((self.sz - 1) as u64),
                |n| n as u64,
            )
    }
}

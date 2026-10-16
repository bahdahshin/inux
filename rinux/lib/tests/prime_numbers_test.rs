// SPDX-License-Identifier: GPL-2.0-only
//! Host tests for the prime sieve, checked against trial division. Linux has a
//! KUnit file for this (`prime_numbers_kunit.c`), which is not ported yet.

use rinux_lib::math::{Primes, slow_is_prime_number, slow_next_prime_number};

#[test]
fn sieve_matches_trial_division_inside_its_range() {
    let mut storage = [0u64; 8]; // 512 numbers
    let primes = Primes::new(&mut storage);
    // From 2 up: C's trial division calls 1 prime, and the sieve does not.
    for n in 2..512u64 {
        assert_eq!(primes.is_prime(n), slow_is_prime_number(n), "n = {n}");
    }
}

#[test]
fn next_prime_walks_the_primes_in_order() {
    let mut storage = [0u64; 8];
    let primes = Primes::new(&mut storage);
    let mut x = 0;
    let mut seen = Vec::new();
    while x < 100 {
        x = primes.next_prime(x);
        seen.push(x);
    }
    assert_eq!(&seen[..6], &[2, 3, 5, 7, 11, 13]);
    assert!(seen.iter().all(|&p| slow_is_prime_number(p)));
}

#[test]
fn past_the_table_falls_back_to_trial_division() {
    let mut storage = [0u64; 1]; // 64 numbers
    let primes = Primes::new(&mut storage);
    assert!(primes.is_prime(1_000_003));
    assert!(!primes.is_prime(1_000_001));
    assert_eq!(primes.next_prime(1000), slow_next_prime_number(1000));
    assert_eq!(slow_next_prime_number(1000), 1009);
}

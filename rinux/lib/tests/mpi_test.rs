// SPDX-License-Identifier: GPL-2.0-or-later
//! Host tests for the MPI limb helpers, checked against 128-bit arithmetic
//! on two-limb numbers and against wider random operands.

use rinux_lib::crypto::mpi::{mpihelp_add_n, mpihelp_cmp, mpihelp_sub_n};

fn value(limbs: &[u64]) -> u128 {
    limbs
        .iter()
        .rev()
        .fold(0u128, |acc, &l| (acc << 64) | u128::from(l))
}

/// A small deterministic generator, so the test needs no RNG.
fn words(seed: &mut u64, n: usize) -> Vec<u64> {
    (0..n)
        .map(|_| {
            *seed = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            *seed
        })
        .collect()
}

#[test]
fn add_matches_128_bit_arithmetic_with_carry() {
    let cases = [
        ([u64::MAX, 0], [1u64, 0]),
        ([u64::MAX, u64::MAX], [1, 0]),
        ([5, 7], [9, 11]),
    ];
    for (a, b) in cases {
        let mut res = [0u64; 2];
        let carry = mpihelp_add_n(&mut res, &a, &b);
        let (sum, overflow) = value(&a).overflowing_add(value(&b));
        assert_eq!(value(&res), sum);
        assert_eq!(u64::from(overflow), carry);
    }
}

#[test]
fn sub_matches_128_bit_arithmetic_with_borrow() {
    let mut seed = 1;
    for _ in 0..200 {
        let a = words(&mut seed, 2);
        let b = words(&mut seed, 2);
        let mut res = [0u64; 2];
        let borrow = mpihelp_sub_n(&mut res, &a, &b);
        let want = value(&a).wrapping_sub(value(&b));
        assert_eq!(value(&res), want);
        assert_eq!(borrow == 1, value(&a) < value(&b));
    }
}

#[test]
fn add_then_sub_round_trips_wide_numbers() {
    let mut seed = 42;
    let a = words(&mut seed, 6);
    let b = words(&mut seed, 6);
    let mut sum = [0u64; 6];
    mpihelp_add_n(&mut sum, &a, &b);
    let mut back = [0u64; 6];
    mpihelp_sub_n(&mut back, &sum, &b);
    // The carry out of the top limb is discarded on both sides, so the
    // round trip returns a exactly, whatever the carry was.
    assert_eq!(back, a[..]);
}

#[test]
fn cmp_is_most_significant_limb_first() {
    assert_eq!(mpihelp_cmp(&[1, 2], &[9, 2]), -1);
    assert_eq!(mpihelp_cmp(&[0, 3], &[u64::MAX, 2]), 1);
    assert_eq!(mpihelp_cmp(&[u64::MAX, 2], &[0, 3]), -1);
    assert_eq!(mpihelp_cmp(&[4, 4], &[4, 4]), 0);
    assert_eq!(mpihelp_cmp(&[5, 4], &[4, 4]), 1);
}

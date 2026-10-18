// SPDX-License-Identifier: GPL-2.0-or-later
//! Host tests for the basecase multiply and squaring, checked against a
//! column-by-column schoolbook product built from u128 partial products.
//! Linux has no KUnit file for these routines.

use rinux_lib::crypto::mpi_sqr::{mpih_sqr_n_basecase, mul_n_basecase};

/// Reference product of `a` and `b`: each pair of limbs adds its 128-bit
/// product at the right limb position, carrying as it goes.
#[allow(clippy::needless_range_loop)]
fn reference_mul(a: &[u64], b: &[u64]) -> Vec<u64> {
    let mut out = vec![0u64; a.len() + b.len()];
    for i in 0..a.len() {
        for j in 0..b.len() {
            let mut carry = u128::from(a[i]) * u128::from(b[j]);
            let mut pos = i + j;
            while carry != 0 {
                let sum = u128::from(out[pos]) + (carry & u128::from(u64::MAX));
                out[pos] = sum as u64;
                carry = (carry >> 64) + (sum >> 64);
                pos += 1;
            }
        }
    }
    out
}

fn words(seed: &mut u64, n: usize) -> Vec<u64> {
    (0..n)
        .map(|i| {
            *seed = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            match i % 4 {
                0 => *seed,
                1 => 0,
                2 => u64::MAX,
                _ => *seed >> 3,
            }
        })
        .collect()
}

#[test]
fn squares_match_the_reference_for_many_sizes() {
    let mut seed = 7;
    for size in 1..=6 {
        for _ in 0..20 {
            let a = words(&mut seed, size);
            let mut prod = vec![0u64; 2 * size];
            mpih_sqr_n_basecase(&mut prod, &a, size);
            assert_eq!(prod, reference_mul(&a, &a), "size {size}: {a:x?}");
        }
    }
}

#[test]
fn products_of_different_operands_match_the_reference() {
    let mut seed = 99;
    for size in 1..=6 {
        for _ in 0..20 {
            let a = words(&mut seed, size);
            let b = words(&mut seed, size);
            let mut prod = vec![0u64; 2 * size];
            mul_n_basecase(&mut prod, &a, &b, size);
            assert_eq!(prod, reference_mul(&a, &b), "size {size}");
        }
    }
}

#[test]
fn small_multipliers_take_the_shortcut_paths() {
    // Limbs 0 and 1 skip the multiply and use a copy or a zero fill.
    let a = [5u64, 9, 7];
    for b in [[0u64, 0, 0], [1, 0, 0], [1, 1, 0], [0, 1, 1]] {
        let mut prod = [0u64; 6];
        mul_n_basecase(&mut prod, &a, &b, 3);
        assert_eq!(prod.to_vec(), reference_mul(&a, &b), "b={b:?}");
    }
}

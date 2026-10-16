// SPDX-License-Identifier: GPL-2.0-or-later
//! Host tests for the basecase squaring, checked against a column-by-column
//! schoolbook product built from u128 partial products. Linux has no KUnit
//! file for this routine.

use rinux_lib::crypto::mpi_sqr::mpih_sqr_n_basecase;

/// Reference product: each pair of limbs adds its 128-bit product at the
/// right limb position, carrying as it goes.
fn reference_square(a: &[u64]) -> Vec<u64> {
    let mut out = vec![0u64; 2 * a.len()];
    for i in 0..a.len() {
        for j in 0..a.len() {
            let mut carry = u128::from(a[i]) * u128::from(a[j]);
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
            assert_eq!(prod, reference_square(&a), "size {size}: {a:x?}");
        }
    }
}

#[test]
fn small_values_take_the_shortcut_paths() {
    // Limbs 0 and 1 skip the multiply and use a copy or a zero fill.
    let a = [1u64, 0, 1];
    let mut prod = [0u64; 6];
    mpih_sqr_n_basecase(&mut prod, &a, 3);
    assert_eq!(prod.to_vec(), reference_square(&a));
}

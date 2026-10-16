// SPDX-License-Identifier: GPL-2.0-or-later
//! Schoolbook multiplication of limb slices, and squaring as its special case.
//!
//! Ported from `mpih_sqr_n_basecase()` in `lib/crypto/mpi/mpih-mul.c`. The
//! C routine multiplies by each limb of `up` in turn, storing the first row
//! and adding the rest. The Karatsuba recursion around it (`mpih_sqr_n`) is
//! not ported here. The row operations are the `mpihelp_mul_1`,
//! `mpihelp_addmul_1` helpers already in rinux-lib; the in-place add is local.

use crate::crypto::mpi_mul::{mpihelp_addmul_1, mpihelp_mul_1};

/// `mpih_sqr_n_basecase()`: `prod = up * up` for `up` of `size` limbs.
/// `prod` must hold `2 * size` limbs.
///
/// Panics if the slice lengths are wrong.
pub fn mpih_sqr_n_basecase(prod: &mut [u64], up: &[u64], size: usize) {
    mul_n_basecase(prod, up, up, size);
}

/// `mul_n_basecase()`: `prod = up * vp` for two `size`-limb operands, the
/// schoolbook product that `mpih_sqr_n_basecase()` specialises to `up * up`.
///
/// Panics if the slice lengths are wrong.
pub fn mul_n_basecase(prod: &mut [u64], up: &[u64], vp: &[u64], size: usize) {
    assert_eq!(up.len(), size, "multiplicand length");
    assert_eq!(vp.len(), size, "multiplier length");
    assert_eq!(prod.len(), 2 * size, "product length");
    if size == 0 {
        return;
    }

    // Multiply by the first limb of U; the result is stored, not added.
    let v0 = vp[0];
    let cy = if v0 <= 1 {
        if v0 == 1 {
            prod[..size].copy_from_slice(up);
        } else {
            prod[..size].fill(0);
        }
        0
    } else {
        mpihelp_mul_1(&mut prod[..size], up, v0)
    };
    prod[size] = cy;

    // Each later row multiplies U by one limb and adds it in at row `i`.
    for i in 1..size {
        let v = vp[i];
        let cy = if v <= 1 {
            if v == 1 {
                add_in_place(&mut prod[i..i + size], up)
            } else {
                0
            }
        } else {
            mpihelp_addmul_1(&mut prod[i..i + size], up, v)
        };
        prod[i + size] = cy;
    }
}

/// `mpihelp_add_n(prodp, prodp, up, size)`: add `src` into `dst` in place,
/// returning the carry. The C call aliases its first two operands, which
/// Rust cannot express, so the carry chain is written out here.
fn add_in_place(dst: &mut [u64], src: &[u64]) -> u64 {
    // Same operation as `mpihelp_add_n` with `res == s1`; `mpihelp_add_n`
    // needs distinct slices, so the carry chain is computed here.
    let mut cy = 0u64;
    for (d, &s) in dst.iter_mut().zip(src) {
        let (s1, o1) = d.overflowing_add(s);
        let (s2, o2) = s1.overflowing_add(cy);
        *d = s2;
        cy = u64::from(o1) + u64::from(o2);
    }
    cy
}

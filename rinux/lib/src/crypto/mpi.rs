// SPDX-License-Identifier: GPL-2.0-or-later
//! Limb-level helpers for multi-precision integers: compare, add and
//! subtract over little-endian arrays of 64-bit limbs.
//!
//! Ported from `lib/crypto/mpi/mpih-cmp.c`, `generic_mpih-add1.c` and
//! `generic_mpih-sub1.c`. The C functions index from the most significant
//! end with negative offsets so the loop is quick. Here they are plain index
//! loops over slices, which compute the same limbs and carries. The `mpi_*`
//! objects built on top of these are not ported.
//!
//! The C pointers are slices, so the sizes come from the slices. All operands
//! of one call must have the same length, as in C.

/// `mpihelp_cmp()`: compare two numbers of the same size, most significant
/// limb first. Returns `1` when `a > b`, `-1` when `a < b` and `0` when equal.
pub fn mpihelp_cmp(a: &[u64], b: &[u64]) -> i32 {
    assert_eq!(a.len(), b.len(), "mpihelp_cmp operands differ in size");
    for (x, y) in a.iter().rev().zip(b.iter().rev()) {
        if x != y {
            return if x > y { 1 } else { -1 };
        }
    }
    0
}

/// `mpihelp_add_n()`: `res = s1 + s2`, limb by limb, returning the carry out
/// of the top limb (`0` or `1`).
pub fn mpihelp_add_n(res: &mut [u64], s1: &[u64], s2: &[u64]) -> u64 {
    assert!(res.len() == s1.len() && s1.len() == s2.len());
    let mut cy = 0u64;
    for j in 0..s1.len() {
        let x = s1[j];
        let mut y = s2[j];
        y = y.wrapping_add(cy); // add previous carry to one addend
        cy = u64::from(y < cy); // carry out of that addition
        y = y.wrapping_add(x); // add the other addend
        cy += u64::from(y < x); // carry out of that add, combined
        res[j] = y;
    }
    cy
}

/// `mpihelp_sub_n()`: `res = s1 - s2`, limb by limb, returning the borrow out
/// of the top limb (`0` or `1`).
pub fn mpihelp_sub_n(res: &mut [u64], s1: &[u64], s2: &[u64]) -> u64 {
    assert!(res.len() == s1.len() && s1.len() == s2.len());
    let mut cy = 0u64;
    for j in 0..s1.len() {
        let x = s1[j];
        let mut y = s2[j];
        y = y.wrapping_add(cy); // add previous carry to the subtrahend
        cy = u64::from(y < cy); // borrow out of that addition
        y = x.wrapping_sub(y); // main subtract
        cy += u64::from(y > x); // borrow out of the subtract, combined
        res[j] = y;
    }
    cy
}

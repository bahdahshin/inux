// SPDX-License-Identifier: GPL-2.0-or-later
//! Multiply a limb slice by one limb: store, add or subtract the product.
//!
//! Ported from `lib/crypto/mpi/generic_mpih-mul1.c`, `generic_mpih-mul2.c`
//! and `generic_mpih-mul3.c`. Each function multiplies `s1` by the single
//! limb `s2`, one limb at a time from the least significant end, and returns
//! the carry limb that does not fit. The C loops count from a negative index
//! for speed; these are plain index loops that compute the same limbs and
//! carries. The 64x64 product is the full 128-bit product, which the C
//! `umul_ppmm()` computes from 32-bit pieces.

/// `mpihelp_mul_1()`: `res = s1 * s2`, returning the carry limb. `res` and
/// `s1` must have the same length.
pub fn mpihelp_mul_1(res: &mut [u64], s1: &[u64], s2: u64) -> u64 {
    assert_eq!(res.len(), s1.len(), "mul_1 operands differ in size");
    let mut cy_limb = 0u64;
    for (r, &a) in res.iter_mut().zip(s1) {
        let (prod_high, mut prod_low) = umul_ppmm_u64(a, s2);
        prod_low = prod_low.wrapping_add(cy_limb);
        cy_limb = u64::from(prod_low < cy_limb) + prod_high;
        *r = prod_low;
    }
    cy_limb
}

/// `mpihelp_addmul_1()`: `res += s1 * s2`, returning the carry limb.
pub fn mpihelp_addmul_1(res: &mut [u64], s1: &[u64], s2: u64) -> u64 {
    assert_eq!(res.len(), s1.len(), "addmul_1 operands differ in size");
    let mut cy_limb = 0u64;
    for (r, &a) in res.iter_mut().zip(s1) {
        let (prod_high, mut prod_low) = umul_ppmm_u64(a, s2);
        prod_low = prod_low.wrapping_add(cy_limb);
        cy_limb = u64::from(prod_low < cy_limb) + prod_high;
        let x = *r;
        prod_low = x.wrapping_add(prod_low);
        cy_limb += u64::from(prod_low < x);
        *r = prod_low;
    }
    cy_limb
}

/// `mpihelp_submul_1()`: `res -= s1 * s2`, returning the borrow limb.
pub fn mpihelp_submul_1(res: &mut [u64], s1: &[u64], s2: u64) -> u64 {
    assert_eq!(res.len(), s1.len(), "submul_1 operands differ in size");
    let mut cy_limb = 0u64;
    for (r, &a) in res.iter_mut().zip(s1) {
        let (prod_high, mut prod_low) = umul_ppmm_u64(a, s2);
        prod_low = prod_low.wrapping_add(cy_limb);
        cy_limb = u64::from(prod_low < cy_limb) + prod_high;
        let x = *r;
        prod_low = x.wrapping_sub(prod_low);
        cy_limb += u64::from(prod_low > x);
        *r = prod_low;
    }
    cy_limb
}

/// The 64x64 product as `(high, low)` limbs, the `umul_ppmm()` of C.
fn umul_ppmm_u64(u: u64, v: u64) -> (u64, u64) {
    let p = u128::from(u) * u128::from(v);
    ((p >> 64) as u64, p as u64)
}

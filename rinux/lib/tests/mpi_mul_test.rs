// SPDX-License-Identifier: GPL-2.0-or-later
//! Host tests for the MPI multiply-by-limb helpers, checked against 128-bit
//! arithmetic written out limb by limb.

use rinux_lib::crypto::mpi_mul::{mpihelp_addmul_1, mpihelp_mul_1, mpihelp_submul_1};

const LIMBS: [u64; 6] = [
    0,
    1,
    0xffff_ffff,
    0x1234_5678_9abc_def0,
    u64::MAX - 1,
    u64::MAX,
];

#[test]
fn mul_1_matches_a_two_limb_reference() {
    for &a0 in &LIMBS {
        for &a1 in &LIMBS {
            for &s in &LIMBS {
                let p0 = u128::from(a0) * u128::from(s);
                let r0 = p0 as u64;
                let c1 = (p0 >> 64) as u64;
                let p1 = u128::from(a1) * u128::from(s) + u128::from(c1);
                let want = [r0, p1 as u64];
                let mut res = [0u64; 2];
                let carry = mpihelp_mul_1(&mut res, &[a0, a1], s);
                assert_eq!(res, want, "[{a0:#x}, {a1:#x}] * {s:#x}");
                assert_eq!(carry, (p1 >> 64) as u64);
            }
        }
    }
}

#[test]
fn addmul_1_adds_the_product_to_one_limb() {
    for &r in &LIMBS {
        for &a in &LIMBS {
            for &s in &LIMBS {
                let total = u128::from(r) + u128::from(a) * u128::from(s);
                let mut res = [r];
                let carry = mpihelp_addmul_1(&mut res, &[a], s);
                assert_eq!(res[0], total as u64, "{r:#x} + {a:#x} * {s:#x}");
                assert_eq!(carry, (total >> 64) as u64);
            }
        }
    }
}

#[test]
fn submul_1_subtracts_the_product_from_one_limb() {
    for &r in &LIMBS {
        for &a in &LIMBS {
            for &s in &LIMBS {
                let p = u128::from(a) * u128::from(s);
                let low = p as u64;
                let high = (p >> 64) as u64;
                let mut res = [r];
                let borrow = mpihelp_submul_1(&mut res, &[a], s);
                assert_eq!(res[0], r.wrapping_sub(low), "{r:#x} - {a:#x} * {s:#x}");
                assert_eq!(borrow, u64::from(low > r) + high);
            }
        }
    }
}

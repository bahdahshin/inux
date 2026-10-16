// SPDX-License-Identifier: GPL-2.0-or-later
//! Host tests for `mpi_sub_ui`, checked against signed 128-bit arithmetic.
//! Linux has no KUnit file for it.

use rinux_lib::crypto::mpi_obj::Mpi;
use rinux_lib::crypto::mpi_sub_ui::mpi_sub_ui;
use rinux_lib::errno::ENOMEM;

/// The signed value of a result with at most two limbs.
fn signed_value(w: &Mpi<'_>) -> i128 {
    let mag = (0..w.nlimbs)
        .rev()
        .fold(0i128, |acc, i| (acc << 64) | i128::from(w.limbs[i]));
    if w.sign { -mag } else { mag }
}

#[test]
fn one_limb_results_match_signed_arithmetic() {
    let values = [0u64, 1, 2, 5, 1 << 40, u64::MAX - 1, u64::MAX];
    let vvals = [0u64, 1, 3, 5, 1 << 40, u64::MAX];
    for &x in &values {
        for &neg in &[false, true] {
            for &v in &vvals {
                let mut u_storage = [x];
                let mut u = Mpi::new(&mut u_storage, usize::from(x != 0), neg && x != 0);
                let mut w_storage = [0u64; 2];
                let mut w = Mpi::new(&mut w_storage, 0, false);
                mpi_sub_ui(&mut w, &u, v).unwrap();

                let u_signed = if u.sign {
                    -i128::from(x)
                } else {
                    i128::from(x)
                };
                let want = u_signed - i128::from(v);
                assert_eq!(signed_value(&w), want, "u={u_signed} v={v}");
                u.normalize();
            }
        }
    }
}

#[test]
fn a_multi_limb_positive_minus_a_small_value() {
    let mut u_storage = [0u64, 1]; // 2^64
    let u = Mpi::new(&mut u_storage, 2, false);
    let mut w_storage = [0u64; 3];
    let mut w = Mpi::new(&mut w_storage, 0, false);
    mpi_sub_ui(&mut w, &u, 1).unwrap();
    assert!(!w.sign);
    assert_eq!(w.nlimbs, 1);
    assert_eq!(w.limbs[0], u64::MAX);
}

#[test]
fn a_multi_limb_negative_adds_magnitudes() {
    let mut u_storage = [u64::MAX, u64::MAX]; // -(2^128 - 1)
    let u = Mpi::new(&mut u_storage, 2, true);
    let mut w_storage = [0u64; 3];
    let mut w = Mpi::new(&mut w_storage, 0, false);
    mpi_sub_ui(&mut w, &u, 1).unwrap();
    assert!(w.sign);
    // |u| + 1 = 2^128, which takes three limbs.
    assert_eq!(w.nlimbs, 3);
    assert_eq!(&w.limbs[..3], &[0, 0, 1]);
}

#[test]
fn storage_is_checked_for_the_carry_limb() {
    let mut u_storage = [0u64, 1];
    let u = Mpi::new(&mut u_storage, 2, false);

    // Two limbs cannot hold the carry case, so the call is refused.
    let mut short_storage = [0u64; 2];
    let mut short = Mpi::new(&mut short_storage, 0, false);
    assert_eq!(mpi_sub_ui(&mut short, &u, 1), Err(ENOMEM));

    let mut ok_storage = [0u64; 3];
    let mut ok = Mpi::new(&mut ok_storage, 0, false);
    assert_eq!(mpi_sub_ui(&mut ok, &u, 1), Ok(()));
}

// SPDX-License-Identifier: GPL-2.0-or-later
//! Host tests for `mpi_add` and `mpi_sub`, checked against signed 128-bit
//! arithmetic over one-limb operands, plus carry and storage edges. Linux
//! has no KUnit file for them.

use rinux_lib::crypto::mpi_add::{mpi_add, mpi_sub};
use rinux_lib::crypto::mpi_obj::Mpi;
use rinux_lib::errno::ENOMEM;

/// The signed value of a result with at most two limbs.
fn signed_value(w: &Mpi<'_>) -> i128 {
    let mag = (0..w.nlimbs)
        .rev()
        .fold(0i128, |acc, i| (acc << 64) | i128::from(w.limbs[i]));
    if w.sign { -mag } else { mag }
}

fn signed(x: u64, neg: bool) -> i128 {
    if neg { -i128::from(x) } else { i128::from(x) }
}

const VALUES: [u64; 6] = [0, 1, 7, 1 << 40, u64::MAX - 1, u64::MAX];

#[test]
fn one_limb_add_and_sub_match_signed_arithmetic() {
    for &x in &VALUES {
        for &y in &VALUES {
            for &xn in &[false, true] {
                for &yn in &[false, true] {
                    let mut ux = [x];
                    let u = Mpi::new(&mut ux, usize::from(x != 0), xn && x != 0);
                    let mut vy = [y];
                    let v = Mpi::new(&mut vy, usize::from(y != 0), yn && y != 0);

                    let mut w_storage = [0u64; 3];
                    let mut w = Mpi::new(&mut w_storage, 0, false);
                    mpi_add(&mut w, &u, &v).unwrap();
                    let want = signed(x, xn && x != 0) + signed(y, yn && y != 0);
                    assert_eq!(signed_value(&w), want, "{x} {xn} + {y} {yn}");

                    let mut w_storage = [0u64; 3];
                    let mut w = Mpi::new(&mut w_storage, 0, false);
                    mpi_sub(&mut w, &u, &v).unwrap();
                    let want = signed(x, xn && x != 0) - signed(y, yn && y != 0);
                    assert_eq!(signed_value(&w), want, "{x} {xn} - {y} {yn}");
                }
            }
        }
    }
}

#[test]
fn positive_carry_grows_into_a_new_limb() {
    let mut ux = [u64::MAX, 0];
    let u = Mpi::new(&mut ux, 1, false);
    let mut vy = [1u64];
    let v = Mpi::new(&mut vy, 1, false);
    let mut w_storage = [0u64; 3];
    let mut w = Mpi::new(&mut w_storage, 0, false);
    mpi_add(&mut w, &u, &v).unwrap();
    assert!(!w.sign);
    assert_eq!(signed_value(&w), i128::from(u64::MAX) + 1);
}

#[test]
fn a_full_result_is_refused_without_writing() {
    let mut ux = [1u64, 2];
    let u = Mpi::new(&mut ux, 2, false);
    let mut vy = [1u64, 1];
    let v = Mpi::new(&mut vy, 2, false);
    let mut short_storage = [0u64; 2];
    let mut short = Mpi::new(&mut short_storage, 0, false);
    assert_eq!(mpi_add(&mut short, &u, &v), Err(ENOMEM));
    assert_eq!(short_storage, [0, 0]);
}

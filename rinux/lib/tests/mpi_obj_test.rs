// SPDX-License-Identifier: GPL-2.0-or-later
//! Host tests for the `Mpi` object and its comparisons. Linux has no KUnit
//! file for `mpi-cmp.c`; these follow its branches one by one.

use rinux_lib::crypto::mpi_obj::{Mpi, mpi_cmp};

#[test]
fn normalize_drops_zero_top_limbs() {
    let mut storage = [5u64, 0, 0];
    let mut m = Mpi::new(&mut storage, 3, false);
    m.normalize();
    assert_eq!(m.nlimbs, 1);
}

#[test]
fn cmp_ui_handles_zero_sign_size_and_limb() {
    let mut zero_storage = [0u64];
    let mut zero = Mpi::new(&mut zero_storage, 0, false);
    assert_eq!(zero.cmp_ui(0), 0);
    assert_eq!(zero.cmp_ui(1), -1);

    let mut neg_storage = [3u64];
    let mut neg = Mpi::new(&mut neg_storage, 1, true);
    assert_eq!(neg.cmp_ui(0), -1);

    let mut big_storage = [1u64, 1];
    let mut big = Mpi::new(&mut big_storage, 2, false);
    assert_eq!(big.cmp_ui(u64::MAX), 1);

    let mut five_storage = [5u64];
    let mut five = Mpi::new(&mut five_storage, 1, false);
    assert_eq!(five.cmp_ui(5), 0);
    assert_eq!(five.cmp_ui(4), 1);
    assert_eq!(five.cmp_ui(6), -1);
}

#[test]
fn cmp_orders_by_sign_then_size_then_limbs() {
    let (mut ps, mut ns) = ([1u64], [1u64]);
    let mut p = Mpi::new(&mut ps, 1, false);
    let mut n = Mpi::new(&mut ns, 1, true);
    // Positive beats negative, negative loses to positive.
    assert_eq!(mpi_cmp(&mut p, &mut n), 1);
    assert_eq!(mpi_cmp(&mut n, &mut p), -1);

    let (mut ts, mut os) = ([0u64, 1], [1u64]);
    let mut two = Mpi::new(&mut ts, 2, false);
    let mut one = Mpi::new(&mut os, 1, false);
    // Same sign, different sizes: the size difference is the result.
    assert_eq!(mpi_cmp(&mut two, &mut one), 1);
    assert_eq!(mpi_cmp(&mut one, &mut two), -1);

    let (mut nts, mut nos) = ([0u64, 1], [1u64]);
    let mut neg_two = Mpi::new(&mut nts, 2, true);
    let mut neg_one = Mpi::new(&mut nos, 1, true);
    // Both negative with different sizes flips the order.
    assert_eq!(mpi_cmp(&mut neg_two, &mut neg_one), -1);

    let (mut as_, mut bs) = ([9u64, 2], [1u64, 2]);
    let mut a = Mpi::new(&mut as_, 2, false);
    let mut b = Mpi::new(&mut bs, 2, false);
    assert_eq!(mpi_cmp(&mut a, &mut b), 1);
    let (mut nas, mut nbs) = ([9u64, 2], [1u64, 2]);
    let mut na = Mpi::new(&mut nas, 2, true);
    let mut nb = Mpi::new(&mut nbs, 2, true);
    // Same size and both negative: the limb order reverses.
    assert_eq!(mpi_cmp(&mut na, &mut nb), -1);
}

#[test]
fn cmp_ignores_leading_zero_limbs() {
    let (mut padded_s, mut plain_s) = ([7u64, 0, 0], [7u64]);
    let mut padded = Mpi::new(&mut padded_s, 3, false);
    let mut plain = Mpi::new(&mut plain_s, 1, false);
    assert_eq!(mpi_cmp(&mut padded, &mut plain), 0);
}

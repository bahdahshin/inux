// SPDX-License-Identifier: GPL-2.0-or-later
//! Host tests for the MPI bit operations, checked against u128 bit
//! arithmetic on two-limb numbers. Linux has no KUnit file for `mpi-bit.c`.

use rinux_lib::crypto::mpi_bit::mpi_rshift;
use rinux_lib::crypto::mpi_obj::Mpi;
use rinux_lib::errno::ENOMEM;

fn value(limbs: &[u64]) -> u128 {
    limbs
        .iter()
        .rev()
        .fold(0u128, |acc, &l| (acc << 64) | u128::from(l))
}

#[test]
fn get_nbits_counts_significant_bits() {
    let mut zero = [0u64];
    assert_eq!(Mpi::new(&mut zero, 1, false).get_nbits(), 0);

    let mut one = [1u64];
    assert_eq!(Mpi::new(&mut one, 1, false).get_nbits(), 1);

    let mut top_byte = [0u64, 0x80];
    assert_eq!(Mpi::new(&mut top_byte, 2, false).get_nbits(), 72);

    let mut high = [0u64, 1];
    assert_eq!(Mpi::new(&mut high, 2, false).get_nbits(), 65);

    let mut full = [u64::MAX, u64::MAX];
    assert_eq!(Mpi::new(&mut full, 2, false).get_nbits(), 128);

    let mut padded = [0x80u64, 0];
    assert_eq!(Mpi::new(&mut padded, 2, false).get_nbits(), 8);
}

#[test]
fn test_bit_reads_inside_and_zero_beyond() {
    let mut limbs = [0b1010u64];
    let m = Mpi::new(&mut limbs, 1, false);
    assert!(!m.test_bit(0));
    assert!(m.test_bit(1));
    assert!(!m.test_bit(2));
    assert!(m.test_bit(3));
    assert!(!m.test_bit(64));
}

#[test]
fn set_bit_grows_within_storage_and_refuses_beyond_it() {
    let mut storage = [0u64, 0];
    let mut m = Mpi::new(&mut storage, 0, false);
    m.set_bit(70).unwrap();
    assert_eq!(m.nlimbs, 2);
    assert!(m.test_bit(70));
    assert!(!m.test_bit(69));

    assert_eq!(m.set_bit(128), Err(ENOMEM));
    assert_eq!(m.nlimbs, 2);
}

#[test]
fn rshift_matches_u128_shifts() {
    let samples = [
        [1u64, 0],
        [u64::MAX, 0],
        [0x1234_5678_9abc_def0, 0x0fed_cba9_8765_4321],
        [u64::MAX, u64::MAX],
    ];
    for up in samples {
        for n in 0..128 {
            let mut a_storage = up;
            let a = Mpi::new(&mut a_storage, 2, false);
            let mut x_storage = [0u64; 2];
            let mut x = Mpi::new(&mut x_storage, 0, false);
            mpi_rshift(&mut x, &a, n).unwrap();
            let want = value(&up) >> n;
            assert_eq!(value(&x.limbs[..x.nlimbs]), want, "{up:x?} >> {n}");
        }
    }
}

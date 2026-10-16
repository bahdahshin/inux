// SPDX-License-Identifier: GPL-2.0-or-later
//! Host tests for the MPI shift helpers, checked against 128-bit arithmetic
//! on two-limb numbers.

use rinux_lib::crypto::mpi_shift::{mpihelp_lshift, mpihelp_rshift};

fn value(limbs: &[u64]) -> u128 {
    limbs
        .iter()
        .rev()
        .fold(0u128, |acc, &l| (acc << 64) | u128::from(l))
}

#[test]
fn lshift_matches_128_bit_shift_and_returns_the_overflow() {
    let samples = [
        [1u64, 0],
        [u64::MAX, 0],
        [0x1234_5678_9abc_def0, 0x0fed_cba9_8765_4321],
        [u64::MAX, u64::MAX],
    ];
    for up in samples {
        for cnt in 1..64 {
            let mut res = [0u64; 2];
            let out = mpihelp_lshift(&mut res, &up, cnt);
            let v = value(&up);
            assert_eq!(value(&res), v << cnt, "{up:x?} << {cnt}");
            assert_eq!(out as u128, v >> (128 - cnt), "overflow {up:x?} << {cnt}");
        }
    }
}

#[test]
fn rshift_matches_128_bit_shift_and_returns_the_bits_dropped() {
    let samples = [
        [1u64, 0],
        [u64::MAX, 0],
        [0x1234_5678_9abc_def0, 0x0fed_cba9_8765_4321],
        [u64::MAX, u64::MAX],
    ];
    for up in samples {
        for cnt in 1..64 {
            let mut res = [0u64; 2];
            let out = mpihelp_rshift(&mut res, &up, cnt);
            let v = value(&up);
            assert_eq!(value(&res), v >> cnt, "{up:x?} >> {cnt}");
            // The dropped bits sit in the high part of the returned limb.
            assert_eq!(out, (v as u64) << (64 - cnt), "dropped {up:x?} >> {cnt}");
        }
    }
}

#[test]
fn shifting_left_then_right_keeps_the_low_bits() {
    let up = [0xdead_beef_0bad_f00d_u64, 0x0123_4567_89ab_cdef, 0x42];
    let mut left = [0u64; 3];
    let out = mpihelp_lshift(&mut left, &up, 17);
    assert_eq!(out >> 47, 0x42 >> 47);
    let mut back = [0u64; 3];
    mpihelp_rshift(&mut back, &left, 17);
    // The top 17 bits of `up` were shifted out and cannot come back.
    assert_eq!(back[0], up[0]);
    assert_eq!(back[1], up[1]);
    assert_eq!(back[2], 0x42 & ((1 << 47) - 1));
}

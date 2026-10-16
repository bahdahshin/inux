// SPDX-License-Identifier: GPL-2.0-or-later
//! Host tests for the RAID-6 port. Linux has no KUnit file for `lib/raid6`'s
//! generic code; these check the field tables against the field axioms, the
//! syndromes against a direct sum, and the recovery against every failure
//! pair of a six-disk stripe.

use rinux_lib::raid6::*;

/// A deterministic byte stream, so failures are reproducible.
fn stream(seed: u32, n: usize) -> Vec<u8> {
    let mut s = seed;
    (0..n)
        .map(|_| {
            s = s.wrapping_mul(1_103_515_245).wrapping_add(12_345);
            (s >> 16) as u8
        })
        .collect()
}

#[test]
fn exp_and_log_are_inverse_powers_of_two() {
    assert_eq!(GFEXP[0], 1);
    assert_eq!(GFEXP[1], 2);
    assert_eq!(GFEXP[8], 0x1d, "2^8 reduces by the polynomial 0x11d");
    assert_eq!(GFLOG[2], 1);
    for i in 0..255 {
        assert_eq!(GFLOG[GFEXP[i] as usize] as usize, i);
    }
    // Entry 255 is not a real power and is left as zero, as in C.
    assert_eq!(GFEXP[255], 0);
}

#[test]
fn multiplication_follows_the_logs() {
    for a in 1..=255u16 {
        for b in 1..=255u16 {
            let (a, b) = (a as u8, b as u8);
            let via_log = GFEXP[(GFLOG[a as usize] as usize + GFLOG[b as usize] as usize) % 255];
            assert_eq!(gfmul(a, b), via_log, "{a} * {b}");
        }
    }
    assert_eq!(gfmul(0, 77), 0);
}

#[test]
fn inverse_and_exi_tables_hold() {
    assert_eq!(GFINV[0], 0);
    for x in 1..=255u8 {
        assert_eq!(gfmul(x, GFINV[x as usize]), 1, "inverse of {x}");
    }
    for x in 1..255usize {
        // GFEXI[x] = inv(2^x + 1); for x = 0 the argument is zero.
        assert_eq!(gfmul(GFEXP[x] ^ 1, GFEXI[x]), 1, "exi of {x}");
    }
}

#[test]
fn gen_syndrome_matches_a_direct_sum() {
    let disks = 6;
    let blocks: Vec<Vec<u8>> = (0..disks - 2).map(|d| stream(d as u32 + 1, 64)).collect();
    let data: Vec<&[u8]> = blocks.iter().map(|b| b.as_slice()).collect();
    let mut p = vec![0u8; 64];
    let mut q = vec![0u8; 64];
    gen_syndrome(&data, &mut p, &mut q);
    for i in 0..64 {
        let mut ep = 0u8;
        let mut eq = 0u8;
        for (d, b) in data.iter().enumerate() {
            ep ^= b[i];
            eq ^= gfmul(GFEXP[d], b[i]);
        }
        assert_eq!(p[i], ep, "P at {i}");
        assert_eq!(q[i], eq, "Q at {i}");
    }
}

#[test]
fn xor_syndrome_adds_ranges_into_the_parity() {
    let blocks: Vec<Vec<u8>> = (0..5).map(|d| stream(100 + d as u32, 32)).collect();
    let data: Vec<&[u8]> = blocks.iter().map(|b| b.as_slice()).collect();
    let mut full_p = vec![0u8; 32];
    let mut full_q = vec![0u8; 32];
    gen_syndrome(&data, &mut full_p, &mut full_q);

    // Build the same parity from two disjoint ranges, starting from zero.
    let mut p = vec![0u8; 32];
    let mut q = vec![0u8; 32];
    xor_syndrome(&data, 0, 1, &mut p, &mut q);
    xor_syndrome(&data, 2, 4, &mut p, &mut q);
    assert_eq!(p, full_p);
    assert_eq!(q, full_q);
}

#[test]
fn two_data_recovery_works_for_every_pair() {
    let n = 4;
    let blocks: Vec<Vec<u8>> = (0..n).map(|d| stream(7 + d as u32, 48)).collect();
    let data: Vec<&[u8]> = blocks.iter().map(|b| b.as_slice()).collect();
    let mut p = vec![0u8; 48];
    let mut q = vec![0u8; 48];
    gen_syndrome(&data, &mut p, &mut q);

    for a in 0..n {
        for b in a + 1..n {
            // Zero the lost disks, then rebuild them from the parity.
            let zero = vec![0u8; 48];
            let lost: Vec<&[u8]> = (0..n)
                .map(|d| {
                    if d == a || d == b {
                        zero.as_slice()
                    } else {
                        blocks[d].as_slice()
                    }
                })
                .collect();
            let mut out_a = vec![0u8; 48];
            let mut out_b = vec![0u8; 48];
            recov_2data(&lost, &p, &q, a, b, &mut out_a, &mut out_b);
            assert_eq!(out_a, blocks[a], "disks {a},{b}: A");
            assert_eq!(out_b, blocks[b], "disks {a},{b}: B");
        }
    }
}

#[test]
fn one_data_recovery_rebuilds_data_and_p() {
    let n = 4;
    let blocks: Vec<Vec<u8>> = (0..n).map(|d| stream(31 + d as u32, 40)).collect();
    let data: Vec<&[u8]> = blocks.iter().map(|b| b.as_slice()).collect();
    let mut p = vec![0u8; 40];
    let mut q = vec![0u8; 40];
    gen_syndrome(&data, &mut p, &mut q);

    for a in 0..n {
        let zero = vec![0u8; 40];
        let lost: Vec<&[u8]> = (0..n)
            .map(|d| {
                if d == a {
                    zero.as_slice()
                } else {
                    blocks[d].as_slice()
                }
            })
            .collect();
        let mut out_a = vec![0u8; 40];
        let mut p_again = vec![0u8; 40];
        recov_datap(&lost, &mut p_again, &q, a, &mut out_a);
        assert_eq!(out_a, blocks[a], "disk {a}");
        assert_eq!(p_again, p, "P after disk {a}");
    }
}

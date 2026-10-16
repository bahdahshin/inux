// SPDX-License-Identifier: GPL-2.0-or-later
//! Host tests for the generic 8-register XOR. The C file has no test; these
//! check each source count against a plain loop, and that a trailing partial
//! group of words is left alone, as the C loop leaves it.

use rinux_lib::raid_xor::*;

fn words(seed: u64, n: usize) -> Vec<u64> {
    (0..n as u64)
        .map(|i| (seed.wrapping_mul(0x9E37_79B9_7F4A_7C15)) ^ i.wrapping_mul(0xC2B2_AE3D_27D4_EB4F))
        .collect()
}

#[test]
fn each_source_count_matches_a_plain_loop() {
    let n = 24;
    let p2 = words(1, n);
    let p3 = words(2, n);
    let p4 = words(3, n);
    let p5 = words(4, n);
    let base = words(9, n);

    let mut a = base.clone();
    xor_2(&mut a, &p2);
    let mut b = base.clone();
    for i in 0..n {
        b[i] ^= p2[i];
    }
    assert_eq!(a, b);

    let mut a = base.clone();
    xor_3(&mut a, &p2, &p3);
    let want: Vec<u64> = (0..n).map(|i| base[i] ^ p2[i] ^ p3[i]).collect();
    assert_eq!(a, want);

    let mut a = base.clone();
    xor_4(&mut a, &p2, &p3, &p4);
    let want: Vec<u64> = (0..n).map(|i| base[i] ^ p2[i] ^ p3[i] ^ p4[i]).collect();
    assert_eq!(a, want);

    let mut a = base.clone();
    xor_5(&mut a, &p2, &p3, &p4, &p5);
    let want: Vec<u64> = (0..n)
        .map(|i| base[i] ^ p2[i] ^ p3[i] ^ p4[i] ^ p5[i])
        .collect();
    assert_eq!(a, want);
}

#[test]
fn trailing_partial_group_is_untouched() {
    // 11 words: one full group of 8, then 3 words the C loop never visits.
    let src = words(5, 11);
    let mut dst = words(6, 11);
    let before = dst.clone();
    xor_2(&mut dst, &src);
    for i in 0..8 {
        assert_eq!(dst[i], before[i] ^ src[i]);
    }
    assert_eq!(&dst[8..], &before[8..]);
}

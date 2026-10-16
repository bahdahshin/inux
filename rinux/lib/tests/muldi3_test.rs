// SPDX-License-Identifier: GPL-2.0-or-later
//! Host tests for the `muldi3` helpers. Linux has no KUnit file for them;
//! these check the halves algorithm against the native operators.

use rinux_lib::muldi3::{muldi3, umul_ppmm, umulsidi3};

const WORDS: [u32; 7] = [0, 1, 2, 0xffff, 0x1_0000, 0x8000_0000, u32::MAX];
const SIGNED: [i64; 9] = [
    0,
    1,
    -1,
    2,
    i64::MAX,
    i64::MIN,
    0x1234_5678_9abc_def0,
    -0x1234_5678_9abc_def0,
    0xffff_ffff,
];

#[test]
fn umul_ppmm_gives_the_full_32_bit_product() {
    for &u in &WORDS {
        for &v in &WORDS {
            let want = u64::from(u) * u64::from(v);
            assert_eq!(umulsidi3(u, v), want, "{u:#x} * {v:#x}");
            let (hi, lo) = umul_ppmm(u, v);
            assert_eq!((u64::from(hi) << 32) | u64::from(lo), want);
        }
    }
}

#[test]
fn muldi3_wraps_like_the_native_multiply() {
    for &u in &SIGNED {
        for &v in &SIGNED {
            assert_eq!(muldi3(u, v), u.wrapping_mul(v), "{u:#x} * {v:#x}");
        }
    }
}

// SPDX-License-Identifier: (GPL-2.0-only OR BSD-3-Clause)
//! Port of `lib/tests/siphash_kunit.c`.
//!
//! Copyright (C) 2016-2022 Jason A. Donenfeld <Jason@zx2c4.com>. All Rights
//! Reserved.
//!
//! Test vectors taken from reference source available at
//! <https://github.com/veorq/SipHash>; extracted from the C test by
//! `scripts/gen_tables.py`.

use rinux_lib::siphash::*;

include!("data/siphash_vectors.rs");

const TEST_KEY_SIPHASH: SiphashKey = SiphashKey {
    key: [0x0706050403020100, 0x0f0e0d0c0b0a0908],
};
const TEST_KEY_HSIPHASH: HsiphashKey = HsiphashKey {
    key: [0x0706050403020100, 0x0f0e0d0c0b0a0908],
};

#[repr(align(8))]
struct Aligned<const N: usize>([u8; N]);

#[test]
fn siphash_test() {
    let mut input = Aligned([0u8; 64]);
    let mut in_unaligned = Aligned([0u8; 65]);

    for i in 0..64u8 {
        let n = i as usize;
        input.0[n] = i;
        in_unaligned.0[n + 1] = i;
        assert_eq!(
            siphash(&input.0[..n], &TEST_KEY_SIPHASH),
            TEST_VECTORS_SIPHASH[n],
            "siphash self-test aligned {}: FAIL",
            n + 1
        );
        assert_eq!(
            siphash(&in_unaligned.0[1..n + 1], &TEST_KEY_SIPHASH),
            TEST_VECTORS_SIPHASH[n],
            "siphash self-test unaligned {}: FAIL",
            n + 1
        );
        assert_eq!(
            hsiphash(&input.0[..n], &TEST_KEY_HSIPHASH),
            TEST_VECTORS_HSIPHASH[n],
            "hsiphash self-test aligned {}: FAIL",
            n + 1
        );
        assert_eq!(
            hsiphash(&in_unaligned.0[1..n + 1], &TEST_KEY_HSIPHASH),
            TEST_VECTORS_HSIPHASH[n],
            "hsiphash self-test unaligned {}: FAIL",
            n + 1
        );
    }
    assert_eq!(
        siphash_1u64(0x0706050403020100, &TEST_KEY_SIPHASH),
        TEST_VECTORS_SIPHASH[8],
        "1u64"
    );
    assert_eq!(
        siphash_2u64(0x0706050403020100, 0x0f0e0d0c0b0a0908, &TEST_KEY_SIPHASH),
        TEST_VECTORS_SIPHASH[16],
        "2u64"
    );
    assert_eq!(
        siphash_3u64(
            0x0706050403020100,
            0x0f0e0d0c0b0a0908,
            0x1716151413121110,
            &TEST_KEY_SIPHASH
        ),
        TEST_VECTORS_SIPHASH[24],
        "3u64"
    );
    assert_eq!(
        siphash_4u64(
            0x0706050403020100,
            0x0f0e0d0c0b0a0908,
            0x1716151413121110,
            0x1f1e1d1c1b1a1918,
            &TEST_KEY_SIPHASH
        ),
        TEST_VECTORS_SIPHASH[32],
        "4u64"
    );
    assert_eq!(
        siphash_1u32(0x03020100, &TEST_KEY_SIPHASH),
        TEST_VECTORS_SIPHASH[4],
        "1u32"
    );
    assert_eq!(
        siphash_2u32(0x03020100, 0x07060504, &TEST_KEY_SIPHASH),
        TEST_VECTORS_SIPHASH[8],
        "2u32"
    );
    assert_eq!(
        siphash_3u32(0x03020100, 0x07060504, 0x0b0a0908, &TEST_KEY_SIPHASH),
        TEST_VECTORS_SIPHASH[12],
        "3u32"
    );
    assert_eq!(
        siphash_4u32(
            0x03020100,
            0x07060504,
            0x0b0a0908,
            0x0f0e0d0c,
            &TEST_KEY_SIPHASH
        ),
        TEST_VECTORS_SIPHASH[16],
        "4u32"
    );
    assert_eq!(
        hsiphash_1u32(0x03020100, &TEST_KEY_HSIPHASH),
        TEST_VECTORS_HSIPHASH[4],
        "h1u32"
    );
    assert_eq!(
        hsiphash_2u32(0x03020100, 0x07060504, &TEST_KEY_HSIPHASH),
        TEST_VECTORS_HSIPHASH[8],
        "h2u32"
    );
    assert_eq!(
        hsiphash_3u32(0x03020100, 0x07060504, 0x0b0a0908, &TEST_KEY_HSIPHASH),
        TEST_VECTORS_HSIPHASH[12],
        "h3u32"
    );
    assert_eq!(
        hsiphash_4u32(
            0x03020100,
            0x07060504,
            0x0b0a0908,
            0x0f0e0d0c,
            &TEST_KEY_HSIPHASH
        ),
        TEST_VECTORS_HSIPHASH[16],
        "h4u32"
    );
}

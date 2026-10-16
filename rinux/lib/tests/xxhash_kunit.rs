// SPDX-License-Identifier: GPL-2.0 OR BSD-2-Clause
//! Tests for xxHash against published vectors and streaming consistency.

use rinux_lib::xxhash::{Xxh64State, xxh32, xxh64};

/// A 39-byte input used by the published xxHash vectors.
fn spam() -> &'static [u8] {
    b"Nobody inspects the spammish repetition"
}

#[test]
fn known_vectors() {
    assert_eq!(xxh32(b"", 0), 0x02cc_5d05);
    assert_eq!(xxh64(b"", 0), 0xef46_db37_51d8_e999);

    assert_eq!(xxh32(b"abc", 0), 0x32d1_53ff);
    assert_eq!(xxh64(b"abc", 0), 0x44bc_2cf5_ad77_0999);

    assert_eq!(xxh32(b"abc", 2_654_435_761), 0xa1ae_7709);
    assert_eq!(xxh64(b"abc", 2_654_435_761), 0x1318_df30_094a_85fd);

    // 39 bytes: exercises the >=32 main loop (xxh64) and >=16 loop (xxh32).
    assert_eq!(xxh32(spam(), 0), 0xe229_3b2f);
    assert_eq!(xxh64(spam(), 0), 0xfbce_a83c_8a37_8bf1);

    // 100 bytes 0..100 with a non-zero seed.
    let b100: [u8; 100] = core::array::from_fn(|i| i as u8);
    assert_eq!(xxh32(&b100, 0x9e37_79b1), 0xfa99_6f24);
    assert_eq!(xxh64(&b100, 0x9e37_79b1), 0x8832_442a_8828_4f11);

    // 256 bytes 0..256.
    let b256: [u8; 256] = core::array::from_fn(|i| i as u8);
    assert_eq!(xxh32(&b256, 0), 0x5944_1253);
    assert_eq!(xxh64(&b256, 0), 0x1fac_be84_06cd_904b);
}

#[test]
fn streaming_matches_oneshot() {
    // The streaming state fed in arbitrary chunks must match the one-shot
    // hash, across lengths that span the buffering, main-loop and tail paths.
    let data: [u8; 300] = core::array::from_fn(|i| (i * 7 + 3) as u8);
    for &seed in &[0u64, 1, 0x9e37_79b1_7f4a_7c15] {
        for len in [0usize, 1, 7, 8, 15, 16, 31, 32, 33, 64, 100, 200, 300] {
            let want = xxh64(&data[..len], seed);
            for &chunk in &[1usize, 3, 8, 16, 32, 50] {
                let mut st = Xxh64State::new(seed);
                let mut off = 0;
                while off < len {
                    let end = (off + chunk).min(len);
                    st.update(&data[off..end]);
                    off = end;
                }
                assert_eq!(st.digest(), want, "len={len} chunk={chunk} seed={seed}");
            }
        }
    }
}

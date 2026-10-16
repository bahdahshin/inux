// SPDX-License-Identifier: GPL-2.0-or-later
//! Tests for Boyer-Moore against a brute-force reference and against KMP.

use rinux_lib::ctype::toupper;
use rinux_lib::ts_bm::TsBm;
use rinux_lib::ts_kmp::TsKmp;

/// Every (non-overlapping-advanced-by-one, i.e. overlapping) BM match start.
fn bm_all(text: &[u8], pattern: &[u8], icase: bool) -> Vec<usize> {
    let mut scratch = vec![0u32; 256 + pattern.len().max(1)];
    let bm = match TsBm::prepare(pattern, &mut scratch, icase) {
        Some(b) => b,
        None => return Vec::new(),
    };
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(m) = bm.find(text, from) {
        out.push(m);
        from = m + 1;
    }
    out
}

/// Same, via KMP, as an independent oracle.
fn kmp_all(text: &[u8], pattern: &[u8], icase: bool) -> Vec<usize> {
    let mut tbl = vec![0u32; pattern.len().max(1)];
    let kmp = match TsKmp::prepare(pattern, &mut tbl, icase) {
        Some(k) => k,
        None => return Vec::new(),
    };
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(m) = kmp.find(text, from) {
        out.push(m);
        from = m + 1;
    }
    out
}

fn naive_all(text: &[u8], pattern: &[u8], icase: bool) -> Vec<usize> {
    let eq = |a: u8, b: u8| {
        if icase {
            toupper(a) == toupper(b)
        } else {
            a == b
        }
    };
    let mut out = Vec::new();
    if pattern.is_empty() || pattern.len() > text.len() {
        return out;
    }
    for i in 0..=text.len() - pattern.len() {
        if (0..pattern.len()).all(|j| eq(text[i + j], pattern[j])) {
            out.push(i);
        }
    }
    out
}

#[test]
fn specific_cases() {
    let text = b"the quick brown fox";
    let find = |pat: &[u8]| {
        let mut s = vec![0u32; 256 + pat.len()];
        TsBm::prepare(pat, &mut s, false).unwrap().find(text, 0)
    };
    assert_eq!(find(b"the"), Some(0));
    assert_eq!(find(b"quick"), Some(4));
    assert_eq!(find(b"fox"), Some(16));
    assert_eq!(find(b"cat"), None);
    assert_eq!(find(b"z"), None);

    assert_eq!(bm_all(b"aaaaa", b"aa", false), vec![0, 1, 2, 3]);
    assert_eq!(bm_all(b"abababab", b"abab", false), vec![0, 2, 4]);

    // A pattern with a repeated suffix exercises the good-shift table.
    assert_eq!(bm_all(b"xabcabcabcy", b"abcabc", false), vec![1, 4]);

    let mut s = [0u32; 257];
    assert!(TsBm::prepare(b"", &mut s, false).is_none());
}

#[test]
fn ignorecase() {
    let mut s = vec![0u32; 256 + 5];
    let bm = TsBm::prepare(b"World", &mut s, true).unwrap();
    assert_eq!(bm.find(b"hello WORLD and world", 0), Some(6));
    assert_eq!(bm.find(b"hello WORLD and world", 7), Some(16));
}

#[test]
fn matches_reference_and_kmp() {
    let mut seed: u32 = 0x2718_2818;
    let mut rng = || {
        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        seed
    };
    for _ in 0..2000 {
        let tlen = (rng() % 45) as usize;
        let plen = (rng() % 6 + 1) as usize;
        let text: Vec<u8> = (0..tlen).map(|_| b'a' + (rng() % 3) as u8).collect();
        let pat: Vec<u8> = (0..plen).map(|_| b'a' + (rng() % 3) as u8).collect();
        for icase in [false, true] {
            let got = bm_all(&text, &pat, icase);
            assert_eq!(
                got,
                naive_all(&text, &pat, icase),
                "text={text:?} pat={pat:?}"
            );
            assert_eq!(got, kmp_all(&text, &pat, icase), "BM vs KMP");
        }
    }
}

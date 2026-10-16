// SPDX-License-Identifier: GPL-2.0-or-later
//! Tests for the Knuth-Morris-Pratt search against a brute-force reference.

use rinux_lib::ctype::toupper;
use rinux_lib::ts_kmp::TsKmp;

/// Collect every (overlapping) match start with KMP.
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

/// Naive reference: every start index where `pattern` occurs in `text`.
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
        let mut tbl = vec![0u32; pat.len()];
        TsKmp::prepare(pat, &mut tbl, false).unwrap().find(text, 0)
    };
    assert_eq!(find(b"the"), Some(0)); // start
    assert_eq!(find(b"quick"), Some(4)); // middle
    assert_eq!(find(b"fox"), Some(16)); // end
    assert_eq!(find(b"cat"), None); // absent
    assert_eq!(find(b"quack"), None); // partial-then-fail

    // Overlapping matches exercise the prefix table.
    assert_eq!(kmp_all(b"aaaaa", b"aa", false), vec![0, 1, 2, 3]);
    assert_eq!(kmp_all(b"abababab", b"abab", false), vec![0, 2, 4]);

    // Empty pattern is rejected (as kmp_init returns -EINVAL).
    let mut t = [0u32; 1];
    assert!(TsKmp::prepare(b"", &mut t, false).is_none());
}

#[test]
fn ignorecase() {
    let mut tbl = vec![0u32; 5];
    let kmp = TsKmp::prepare(b"World", &mut tbl, true).unwrap();
    assert_eq!(kmp.find(b"hello WORLD and world", 0), Some(6));
    assert_eq!(kmp.find(b"hello WORLD and world", 7), Some(16));
}

#[test]
fn matches_naive_reference() {
    // A small alphabet makes partial matches (and thus the prefix table)
    // common. Deterministic LCG so the test is reproducible.
    let mut seed: u32 = 0x1234_5678;
    let mut rng = || {
        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        seed
    };
    for _ in 0..2000 {
        let tlen = (rng() % 40) as usize;
        let plen = (rng() % 6 + 1) as usize;
        let text: Vec<u8> = (0..tlen).map(|_| b'a' + (rng() % 3) as u8).collect();
        let pat: Vec<u8> = (0..plen).map(|_| b'a' + (rng() % 3) as u8).collect();
        for icase in [false, true] {
            assert_eq!(
                kmp_all(&text, &pat, icase),
                naive_all(&text, &pat, icase),
                "text={text:?} pat={pat:?} icase={icase}"
            );
        }
    }
}

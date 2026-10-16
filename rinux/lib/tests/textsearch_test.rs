// SPDX-License-Identifier: GPL-2.0-or-later
//! Host tests for the `textsearch` front end in `lib/textsearch.c`. Linux's
//! framework has no KUnit file of its own; these cover the name lookup, the
//! flags, and the three algorithms behind it.

use rinux_lib::errno::{EINVAL, ENOENT};
use rinux_lib::textsearch::{TS_IGNORECASE, TextSearch};
use rinux_lib::ts_fsm::{TsFsmRecur, TsFsmToken, TsFsmType};

const ALGOS: [&[u8]; 2] = [b"kmp", b"bm"];

#[test]
fn every_byte_algorithm_finds_the_same_offset() {
    for algo in ALGOS {
        let mut scratch = vec![0u32; 300];
        let ts = TextSearch::prepare(algo, b"abc", &mut scratch, 0).unwrap();
        assert_eq!(ts.find(b"xxabcx", 0), Some(2), "{algo:?}");
        assert_eq!(ts.find(b"xxabcx", 3), None, "{algo:?}");
        assert_eq!(ts.find(b"xyz", 0), None, "{algo:?}");
    }
}

#[test]
fn ignorecase_folds_letters_for_the_byte_algorithms() {
    for algo in ALGOS {
        let mut scratch = vec![0u32; 300];
        let ts = TextSearch::prepare(algo, b"ABC", &mut scratch, TS_IGNORECASE).unwrap();
        assert_eq!(ts.find(b"xabc", 0), Some(1), "{algo:?}");
    }
}

#[test]
fn an_unknown_algorithm_is_enoent() {
    let mut scratch = vec![0u32; 8];
    let err = TextSearch::prepare(b"nope", b"a", &mut scratch, 0).err();
    assert_eq!(err, Some(ENOENT));
}

#[test]
fn an_empty_pattern_or_short_scratch_is_einval() {
    let mut scratch = vec![0u32; 300];
    assert_eq!(
        TextSearch::prepare(b"kmp", b"", &mut scratch, 0).err(),
        Some(EINVAL)
    );
    let mut short: Vec<u32> = vec![];
    assert_eq!(
        TextSearch::prepare(b"kmp", b"abc", &mut short, 0).err(),
        Some(EINVAL)
    );
}

#[test]
fn the_fsm_takes_tokens_and_reports_the_match_start() {
    let toks = [
        TsFsmToken {
            ttype: TsFsmType::Wildcard,
            recur: TsFsmRecur::HeadIgnore,
            value: 0,
        },
        TsFsmToken {
            ttype: TsFsmType::Digit,
            recur: TsFsmRecur::Multi,
            value: 0,
        },
        TsFsmToken {
            ttype: TsFsmType::Specific,
            recur: TsFsmRecur::Single,
            value: b';',
        },
    ];
    let ts = TextSearch::fsm(&toks, 0).unwrap();
    assert_eq!(ts.find(b"x12;", 0), Some(1));
}

#[test]
fn the_fsm_rejects_ignorecase_and_byte_prepare() {
    let toks = [TsFsmToken {
        ttype: TsFsmType::Specific,
        recur: TsFsmRecur::Single,
        value: b'a',
    }];
    assert_eq!(TextSearch::fsm(&toks, TS_IGNORECASE).err(), Some(EINVAL));
    let mut scratch = vec![0u32; 8];
    assert_eq!(
        TextSearch::prepare(b"fsm", b"a", &mut scratch, 0).err(),
        Some(EINVAL)
    );
}

// SPDX-License-Identifier: GPL-2.0-or-later
//! Host tests for `lib/ts_fsm.c`. Linux has no KUnit file for it, so these
//! cover each repeat kind, strict and head-ignoring search, and the pattern
//! checks in `fsm_init()`.

use rinux_lib::errno::EINVAL;
use rinux_lib::ts_fsm::{TsFsm, TsFsmRecur, TsFsmToken, TsFsmType};

fn lit(value: u8, recur: TsFsmRecur) -> TsFsmToken {
    TsFsmToken {
        ttype: TsFsmType::Specific,
        recur,
        value,
    }
}

fn class(ttype: TsFsmType, recur: TsFsmRecur) -> TsFsmToken {
    TsFsmToken {
        ttype,
        recur,
        value: 0,
    }
}

/// Skip any bytes until the next token matches, the usual search head.
fn head() -> TsFsmToken {
    class(TsFsmType::Wildcard, TsFsmRecur::HeadIgnore)
}

#[test]
fn strict_pattern_must_match_at_the_start() {
    let toks = [lit(b'a', TsFsmRecur::Single), lit(b'b', TsFsmRecur::Single)];
    let fsm = TsFsm::new(&toks).unwrap();
    assert_eq!(fsm.find(b"ab", 0), Some((0, 2)));
    assert_eq!(
        fsm.find(b"xab", 0),
        None,
        "strict: a mismatch at the start ends it"
    );
    assert_eq!(fsm.find(b"ac", 0), None);
}

#[test]
fn head_ignore_finds_the_first_match_anywhere() {
    let toks = [
        head(),
        lit(b'a', TsFsmRecur::Single),
        lit(b'b', TsFsmRecur::Single),
    ];
    let fsm = TsFsm::new(&toks).unwrap();
    assert_eq!(fsm.find(b"xxab", 0), Some((2, 4)));
    assert_eq!(fsm.find(b"abab", 1), Some((2, 4)), "search can start later");
    assert_eq!(fsm.find(b"xyz", 0), None);
}

#[test]
fn perhaps_matches_with_or_without_the_optional_byte() {
    let toks = [
        lit(b'a', TsFsmRecur::Perhaps),
        lit(b'b', TsFsmRecur::Single),
    ];
    let fsm = TsFsm::new(&toks).unwrap();
    assert_eq!(fsm.find(b"b", 0), Some((0, 1)));
    assert_eq!(fsm.find(b"ab", 0), Some((0, 2)));
    assert_eq!(fsm.find(b"c", 0), None);
}

#[test]
fn multi_needs_at_least_one_then_takes_a_run() {
    let toks = [
        head(),
        class(TsFsmType::Digit, TsFsmRecur::Multi),
        lit(b';', TsFsmRecur::Single),
    ];
    let fsm = TsFsm::new(&toks).unwrap();
    assert_eq!(fsm.find(b"x12;", 0), Some((1, 4)));
    assert_eq!(
        fsm.find(b"x;", 0),
        None,
        "zero digits is not enough for Multi"
    );
    assert_eq!(fsm.find(b"7;", 0), Some((0, 2)));
}

#[test]
fn any_takes_zero_or_more_before_the_next_token() {
    let toks = [
        lit(b'a', TsFsmRecur::Single),
        class(TsFsmType::Digit, TsFsmRecur::Any),
        lit(b'z', TsFsmRecur::Single),
    ];
    let fsm = TsFsm::new(&toks).unwrap();
    assert_eq!(fsm.find(b"a123z", 0), Some((0, 5)));
    assert_eq!(fsm.find(b"az", 0), Some((0, 2)), "zero digits is allowed");
    assert_eq!(fsm.find(b"a1x", 0), None);
}

#[test]
fn a_trailing_any_ends_the_match_without_consuming() {
    // As in C, an `Any` token with no token after it ends the match at once,
    // so it takes zero bytes even when digits follow.
    let toks = [
        lit(b'a', TsFsmRecur::Single),
        class(TsFsmType::Digit, TsFsmRecur::Any),
    ];
    let fsm = TsFsm::new(&toks).unwrap();
    assert_eq!(fsm.find(b"a12", 0), Some((0, 1)));
    assert_eq!(fsm.find(b"a", 0), Some((0, 1)));
}

#[test]
fn literal_match_is_case_sensitive() {
    let toks = [lit(b'a', TsFsmRecur::Single)];
    let fsm = TsFsm::new(&toks).unwrap();
    assert_eq!(fsm.find(b"A", 0), None);
}

#[test]
fn class_tokens_follow_the_ctype_sets() {
    let toks = [
        class(TsFsmType::Alpha, TsFsmRecur::Single),
        class(TsFsmType::Space, TsFsmRecur::Single),
    ];
    let fsm = TsFsm::new(&toks).unwrap();
    assert_eq!(fsm.find(b"q ", 0), Some((0, 2)));
    assert_eq!(fsm.find(b"1 ", 0), None);
    assert_eq!(fsm.find(b"q9", 0), None);
}

#[test]
fn a_start_at_or_past_the_end_finds_nothing() {
    let toks = [lit(b'a', TsFsmRecur::Single)];
    let fsm = TsFsm::new(&toks).unwrap();
    assert_eq!(fsm.find(b"a", 1), None);
    assert_eq!(fsm.find(b"", 0), None);
}

#[test]
fn the_pattern_is_returned_as_given() {
    let toks = [lit(b'a', TsFsmRecur::Single)];
    assert_eq!(TsFsm::new(&toks).unwrap().pattern(), &toks[..]);
}

#[test]
fn an_empty_pattern_is_rejected() {
    assert_eq!(TsFsm::new(&[]).err(), Some(EINVAL));
}

#[test]
fn head_ignore_is_only_valid_at_the_front_of_a_longer_pattern() {
    let only_head = [head()];
    assert_eq!(TsFsm::new(&only_head).err(), Some(EINVAL));

    let middle = [
        lit(b'a', TsFsmRecur::Single),
        head(),
        lit(b'b', TsFsmRecur::Single),
    ];
    assert_eq!(TsFsm::new(&middle).err(), Some(EINVAL));
}

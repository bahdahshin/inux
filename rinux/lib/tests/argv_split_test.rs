// SPDX-License-Identifier: GPL-2.0-or-later
//! Host tests for `argv_split`. Linux has no KUnit file for it, so these
//! cover the splitting rules the C comment documents.

use rinux_lib::argv_split::{argv_split, count_argc};

fn words(s: &[u8]) -> Vec<&[u8]> {
    argv_split(s).map(|r| &s[r]).collect()
}

#[test]
fn splits_on_runs_of_whitespace() {
    let s = b"  one\ttwo \n\n three  ";
    assert_eq!(words(s), [&b"one"[..], b"two", b"three"]);
    assert_eq!(count_argc(s), 3);
}

#[test]
fn empty_and_blank_strings_have_no_words() {
    assert_eq!(count_argc(b""), 0);
    assert_eq!(count_argc(b" \t\n "), 0);
    assert!(words(b"").is_empty());
}

#[test]
fn a_nul_ends_the_string() {
    assert_eq!(words(b"a b\0c d"), [&b"a"[..], b"b"]);
}

#[test]
fn quotes_are_not_special() {
    assert_eq!(words(b"\"a b\""), [&b"\"a"[..], b"b\""]);
}

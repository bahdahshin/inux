// SPDX-License-Identifier: MIT OR GPL-2.0
//! Port of `lib/tests/glob_kunit.c`.

use rinux_lib::glob::{glob_match, glob_match_len};

/// Pattern, string, expected result.
const GLOB_TEST_CASES: &[(&str, &str, bool)] = &[
    // Some basic tests.
    ("a", "a", true),
    ("a", "b", false),
    ("a", "aa", false),
    ("a", "", false),
    ("", "", true),
    ("", "a", false),
    // Simple character class tests.
    ("[a]", "a", true),
    ("[a]", "b", false),
    ("[!a]", "a", false),
    ("[!a]", "b", true),
    ("[ab]", "a", true),
    ("[ab]", "b", true),
    ("[ab]", "c", false),
    ("[!ab]", "c", true),
    ("[a-c]", "b", true),
    ("[a-c]", "d", false),
    // Corner cases in character class parsing.
    ("[a-c-e-g]", "-", true),
    ("[a-c-e-g]", "d", false),
    ("[a-c-e-g]", "f", true),
    ("[]a-ceg-ik[]", "a", true),
    ("[]a-ceg-ik[]", "]", true),
    ("[]a-ceg-ik[]", "[", true),
    ("[]a-ceg-ik[]", "h", true),
    ("[]a-ceg-ik[]", "f", false),
    ("[!]a-ceg-ik[]", "h", false),
    ("[!]a-ceg-ik[]", "]", false),
    ("[!]a-ceg-ik[]", "f", true),
    // Simple wild cards.
    ("?", "a", true),
    ("?", "aa", false),
    ("??", "a", false),
    ("?x?", "axb", true),
    ("?x?", "abx", false),
    ("?x?", "xab", false),
    // Asterisk wild cards (backtracking).
    ("*??", "a", false),
    ("*??", "ab", true),
    ("*??", "abc", true),
    ("*??", "abcd", true),
    ("??*", "a", false),
    ("??*", "ab", true),
    ("??*", "abc", true),
    ("??*", "abcd", true),
    ("?*?", "a", false),
    ("?*?", "ab", true),
    ("?*?", "abc", true),
    ("?*?", "abcd", true),
    ("*b", "b", true),
    ("*b", "ab", true),
    ("*b", "ba", false),
    ("*b", "bb", true),
    ("*b", "abb", true),
    ("*b", "bab", true),
    ("*bc", "abbc", true),
    ("*bc", "bc", true),
    ("*bc", "bbc", true),
    ("*bc", "bcbc", true),
    // Multiple asterisks (complex backtracking).
    ("*ac*", "abacadaeafag", true),
    ("*ac*ae*ag*", "abacadaeafag", true),
    ("*a*b*[bc]*[ef]*g*", "abacadaeafag", true),
    ("*a*b*[ef]*[cd]*g*", "abacadaeafag", false),
    ("*abcd*", "abcabcabcabcdefg", true),
    ("*ab*cd*", "abcabcabcabcdefg", true),
    ("*abcd*abcdef*", "abcabcdabcdeabcdefg", true),
    ("*abcd*", "abcabcabcabcefg", false),
    ("*ab*cd*", "abcabcabcabcefg", false),
];

#[test]
fn glob_test_match() {
    for &(pat, s, expected) in GLOB_TEST_CASES {
        assert_eq!(
            glob_match(pat.as_bytes(), s.as_bytes()),
            expected,
            "pat:{pat:?} str:{s:?}"
        );
    }
}

// Not in the C test.

#[test]
fn glob_escapes_and_malformed_classes() {
    assert!(glob_match(br"\*", b"*"));
    assert!(!glob_match(br"\*", b"a"));
    assert!(glob_match(br"a\?c", b"a?c"));
    // An unclosed bracket matches literally.
    assert!(glob_match(b"[ab", b"[ab"));
    assert!(!glob_match(b"[ab", b"a"));
    assert!(glob_match(b"*.[ch]", b"main.c"));
    assert!(!glob_match(b"*.[ch]", b"main.rs"));
}

#[test]
fn glob_match_len_stops_early() {
    assert!(glob_match_len(b"abc", b"abcdef", 3));
    assert!(!glob_match_len(b"abcd", b"abcdef", 3));
    assert!(glob_match_len(b"ab*", b"abcdef", 6));
    assert!(glob_match_len(b"ab", b"ab\0cd", 5));
}

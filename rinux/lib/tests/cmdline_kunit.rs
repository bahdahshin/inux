// SPDX-License-Identifier: GPL-2.0+
//! Port of `lib/tests/cmdline_kunit.c`.
//!
//! The C test prefixes or appends one random `u8`; this runs all 256. The
//! `next_arg()` cases run against both [`next_arg_in_place`], the C
//! version, and the slice-returning [`next_arg`].

use rinux_lib::cmdline::{get_option, get_options, memparse, next_arg, next_arg_in_place};
use rinux_lib::sprintf;
use rinux_lib::string::strlen;

const CMDLINE_TEST_STRINGS: [&str; 16] = [
    "\"\"", "", "=", "\"-", ",", "-,", ",-", "-", "+,", "--", ",,", "''", "\"\",", "\",\"",
    "-\"\"", "\"",
];

const CMDLINE_TEST_VALUES: [i32; 16] = [1, 1, 1, 1, 2, 3, 2, 3, 1, 3, 2, 1, 1, 1, 3, 1];

const CMDLINE_TEST_RANGE_STRINGS: [&str; 20] = [
    "-7", "--7", "-1-2", "7--9", "7-", "-7--9", "7-9,", "9-7", "5-a", "a-5", "5-8", ",8-5", "+,1",
    "-,4", "-3,0-1,6", "4,-", " +2", " -9", "0-1,-3,6", "- 9",
];

const CMDLINE_TEST_RANGE_VALUES: [[i32; 16]; 20] = [
    row(&[1, -7]),
    row(&[0, -0]),
    row(&[4, -1, 0, 1, 2]),
    row(&[0, 7]),
    row(&[0, 7]),
    row(&[0, -7]),
    row(&[3, 7, 8, 9, 0]),
    row(&[0, 9]),
    row(&[0, 5]),
    row(&[0, -0]),
    row(&[4, 5, 6, 7, 8]),
    row(&[0, 0]),
    row(&[0, 0]),
    row(&[0, -0]),
    row(&[4, -3, 0, 1, 6]),
    row(&[1, 4]),
    row(&[0, 0]),
    row(&[0, -0]),
    row(&[4, 0, 1, -3, 6]),
    row(&[0, 0]),
];

/// A 16-entry row with the rest zero, as C initializers give.
const fn row(v: &[i32]) -> [i32; 16] {
    let mut r = [0; 16];
    let mut i = 0;
    while i < v.len() {
        r[i] = v[i];
        i += 1;
    }
    r
}

/// The C string in `buf` from `i` on.
fn cstr(buf: &[u8], i: usize) -> &[u8] {
    let s = &buf[i..];
    &s[..strlen(s)]
}

fn cmdline_do_one_test(input: &[u8], rc: i32, offset: usize) {
    let pattern = String::from_utf8_lossy(input);
    let mut out = input;
    let mut dummy = 0;

    let ret = get_option(&mut out, Some(&mut dummy));

    assert_eq!(ret, rc, "Pattern: {pattern}");
    assert_eq!(input.len() - out.len(), offset, "Pattern: {pattern}");
}

#[test]
fn cmdline_test_noint() {
    for s in CMDLINE_TEST_STRINGS {
        // Only first and leading '-' will advance the pointer.
        let offset = s.starts_with('-') as usize;
        cmdline_do_one_test(s.as_bytes(), 0, offset);
    }
}

#[test]
fn cmdline_test_lead_int() {
    for (s, rc) in CMDLINE_TEST_STRINGS.into_iter().zip(CMDLINE_TEST_VALUES) {
        for n in 0..=u8::MAX {
            let mut input = [0u8; 32];
            let len = sprintf!(&mut input, "%u%s", n, s);
            // Only first '-' after the number will advance the pointer.
            let offset = len - s.len() + (rc == 2) as usize;
            cmdline_do_one_test(&input[..len], rc, offset);
        }
    }
}

#[test]
fn cmdline_test_tail_int() {
    for s in CMDLINE_TEST_STRINGS {
        // When "" or "-" the result will be valid integer.
        let rc = if s.is_empty() || s == "-" { 1 } else { 0 };
        for n in 0..=u8::MAX {
            let mut input = [0u8; 32];
            let len = sprintf!(&mut input, "%s%u", s, n);
            // Only first and leading '-' not followed by integer will
            // advance the pointer.
            let offset = if rc != 0 {
                len
            } else {
                s.starts_with('-') as usize
            };
            cmdline_do_one_test(&input[..len], rc, offset);
        }
    }
}

fn cmdline_do_one_range_test(input: &str, n: usize, e: &[i32; 16]) {
    let mut r = [0i32; 16];

    get_options(input.as_bytes(), r.len() as i32, &mut r);
    assert_eq!(
        r[0], e[0],
        "in test {n} (parsed) expected {} numbers, got {}",
        e[0], r[0]
    );
    for i in 1..r.len() {
        assert_eq!(r[i], e[i], "in test {n} at {i}");
    }

    let mut r = [0i32; 16];
    get_options(input.as_bytes(), 0, &mut r);
    assert_eq!(
        r[0], e[0],
        "in test {n} (validated) expected {} numbers, got {}",
        e[0], r[0]
    );

    let p = r[1..].iter().position(|&v| v != 0);
    assert_eq!(p, None, "in test {n} out of bound");
}

#[test]
fn cmdline_test_range() {
    for (i, (s, e)) in CMDLINE_TEST_RANGE_STRINGS
        .iter()
        .zip(&CMDLINE_TEST_RANGE_VALUES)
        .enumerate()
    {
        cmdline_do_one_range_test(s, i, e);
    }
}

/// One `next_arg()` result: parameter, value, rest.
type Split<'a> = (&'a [u8], Option<&'a [u8]>, &'a [u8]);

/// Checks `next_arg_in_place()` on a copy of `input` and `next_arg()` on
/// `input` against the expected splits, one call per entry.
#[track_caller]
fn check_next_arg(input: &[u8], expected: &[Split]) {
    let mut buf = input.to_vec();
    buf.push(0);
    let mut next = 0;
    let mut rest = input;
    for &(param, val, after) in expected {
        let a = next_arg_in_place(&mut buf, next);
        assert_eq!(cstr(&buf, a.param), param);
        assert_eq!(a.val.map(|v| cstr(&buf, v)), val);
        assert_eq!(cstr(&buf, a.next), after);
        next = a.next;

        let (p, v, r) = next_arg(rest);
        assert_eq!((p, v, r), (param, val, after));
        rest = r;
    }
}

#[test]
fn cmdline_test_next_arg_quoted_value() {
    check_next_arg(
        b"foo=\"bar baz\" qux=1",
        &[
            (b"foo", Some(b"bar baz"), b"qux=1"),
            (b"qux", Some(b"1"), b""),
        ],
    );
}

#[test]
fn cmdline_test_next_arg_bare_quote_regression() {
    check_next_arg(
        b"foo=bar \"",
        &[
            (b"foo", Some(b"bar"), b"\""),
            // This hits the i == 0 quoted-token case fixed by 9847f21225c4.
            (b"", None, b""),
        ],
    );
}

#[test]
fn cmdline_test_next_arg_mixed_tokens() {
    check_next_arg(
        b"bbb= jjj kkk=\"a=b\"",
        &[
            (b"bbb", Some(b""), b"jjj kkk=\"a=b\""),
            (b"jjj", None, b"kkk=\"a=b\""),
            (b"kkk", Some(b"a=b"), b""),
        ],
    );
}

/// Not in the C test: `next_arg()` and `next_arg_in_place()` agree on
/// every split of a set of awkward command lines.
#[test]
fn next_arg_slices_match_in_place() {
    let lines: &[&[u8]] = &[
        b"",
        b"   ",
        b"a",
        b"=a",
        b"a=",
        b"a==b",
        b"\"a b\"=c d",
        b"\"a=b c\" d",
        b"\"a\"=b",
        b"a=\"b",
        b"a=\"\"",
        b"a=\"",
        b"\"",
        b"\"\"",
        b"\"a",
        b"a\"b c\"d=e f",
        b"x=\"1 2\"   y=3\t\tz",
        b"console=ttyS0,115200 earlyprintk=serial loglevel=7 -- init args",
        b"a b\0c d",
    ];
    for &line in lines {
        let mut buf = line.to_vec();
        buf.push(0);
        let mut next = 0;
        let mut rest = line;
        for _ in 0..8 {
            let a = next_arg_in_place(&mut buf, next);
            let (p, v, r) = next_arg(rest);
            let what = String::from_utf8_lossy(line);
            assert_eq!(p, cstr(&buf, a.param), "{what}");
            assert_eq!(v, a.val.map(|v| cstr(&buf, v)), "{what}");
            assert_eq!(r, cstr(&buf, a.next), "{what}");
            if r.is_empty() {
                break;
            }
            next = a.next;
            rest = r;
        }
    }
}

const SZ_4K: u64 = 0x1000;
const SZ_16K: u64 = 0x4000;
const SZ_1M: u64 = 0x100000;
const SZ_32M: u64 = 0x02000000;
const SZ_1G: u64 = 0x40000000;
const SZ_2G: u64 = 0x80000000;
const SZ_1T: u64 = 0x10000000000;
const SZ_4T: u64 = 0x40000000000;

/// Input, the unrecognized rest, result.
const TESTDATA: &[(&str, &str, u64)] = &[
    ("0", "", 0),
    ("1", "", 1),
    ("a", "a", 0),
    ("k", "k", 0),
    ("E", "E", 0),
    ("0xb", "", 11),
    ("0xz", "x", 0),
    ("1234", "", 1234),
    ("04567", "", 2423),
    ("0x9876", "", 39030),
    ("05678", "8", 375),
    ("0xabcdefz", "z", 11259375),
    ("0cdba", "c", 0),
    ("4K", "", SZ_4K),
    ("0x10k@0xaaaabbbb", "@", SZ_16K),
    ("32M", "", SZ_32M),
    ("067m:foo", ":", 55 * SZ_1M),
    ("2G;bar=baz", ";", SZ_2G),
    ("07gz", "z", 7 * SZ_1G),
    ("3T+data", "+", 3 * SZ_1T),
    ("04t,ro", ",", SZ_4T),
    ("012p", "", 11258999068426240),
    ("7P,sync", ",", 7881299347898368),
    ("0x2e", "", 46),
    ("2E and more", " ", 2305843009213693952),
    ("18446744073709551615", "", u64::MAX),
    ("0xffffffffffffffff0", "", u64::MAX),
    ("1111111111111111111T", "", u64::MAX),
    ("222222222222222222222G", "", u64::MAX),
    ("3333333333333333333333M", "", u64::MAX),
];

#[test]
fn cmdline_test_memparse() {
    for &(input, unrecognized, result) in TESTDATA {
        let (ret, retptr) = memparse(input.as_bytes());
        assert_eq!(ret, result, "    when parsing '{input}'");
        let rest = input.as_bytes().get(retptr).copied().unwrap_or(0);
        assert_eq!(
            rest,
            unrecognized.as_bytes().first().copied().unwrap_or(0),
            "    when parsing '{input}'"
        );
    }
}

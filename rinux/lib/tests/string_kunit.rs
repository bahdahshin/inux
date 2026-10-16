// SPDX-License-Identifier: GPL-2.0-only
//! Port of `lib/tests/string_kunit.c`. The benchmark cases are not ported.

use rinux_lib::errno::E2BIG;
use rinux_lib::string::*;

const STRCMP_LARGE_BUF_LEN: usize = 2048;
const STRCMP_CHANGE_POINT: usize = 1337;

const STRING_TEST_MAX_LEN: usize = 128;
const STRING_TEST_MAX_OFFSET: usize = 16;
const PAGE_SIZE: usize = 4096;

fn page_align(x: usize) -> usize {
    x.div_ceil(PAGE_SIZE) * PAGE_SIZE
}

macro_rules! expect_equal {
    ($r:expr) => {
        assert_eq!($r, 0, "{}", stringify!($r))
    };
}
macro_rules! expect_lower {
    ($r:expr) => {
        assert!($r < 0, "{}", stringify!($r))
    };
}
macro_rules! expect_greater {
    ($r:expr) => {
        assert!($r > 0, "{}", stringify!($r))
    };
}

/// `KUNIT_EXPECT_STREQ` for a C-string buffer.
fn streq(buf: &[u8], s: &str) -> bool {
    strcmp(buf, s.as_bytes()) == 0
}

macro_rules! memset_test {
    ($name:ident, $f:ident, $ty:ty, $v:expr) => {
        #[test]
        fn $name() {
            let mut p: [$ty; 512] = [0; 512];
            let fill = <$ty>::from_ne_bytes([0xa1; size_of::<$ty>()]);
            for i in 0..256 {
                for j in 0..256 {
                    p.fill(fill);
                    $f(&mut p[i..], $v, j);
                    for (k, &v) in p.iter().enumerate() {
                        let want = if k >= i && k < i + j { $v } else { fill };
                        assert_eq!(v, want, "i:{i} j:{j} k:{k}");
                    }
                }
            }
        }
    };
}

memset_test!(string_test_memset16, memset16, u16, 0xb1b2);
memset_test!(string_test_memset32, memset32, u32, 0xb1b2_b3b4);
memset_test!(string_test_memset64, memset64, u64, 0xb1b2_b3b4_b5b6_b7b8);

#[test]
fn string_test_strlen() {
    let buf_size = page_align(STRING_TEST_MAX_LEN + STRING_TEST_MAX_OFFSET + 1);
    let mut buf = vec![b'A'; buf_size];
    for offset in 0..STRING_TEST_MAX_OFFSET {
        for len in 0..=STRING_TEST_MAX_LEN {
            let s = buf_size - 1 - offset - len;
            buf[s + len] = 0;
            assert_eq!(strlen(&buf[s..]), len, "offset:{offset} len:{len}");
            buf[s + len] = b'A';
        }
    }
}

#[test]
fn string_test_strnlen() {
    let buf_size = page_align(STRING_TEST_MAX_LEN + STRING_TEST_MAX_OFFSET + 1);
    let mut buf = vec![b'A'; buf_size];
    for offset in 0..STRING_TEST_MAX_OFFSET {
        for len in 0..=STRING_TEST_MAX_LEN {
            let s = buf_size - 1 - offset - len;
            buf[s + len] = 0;
            let str = &buf[s..];
            if len > 0 {
                assert_eq!(strnlen(str, len - 1), len - 1);
            }
            if len > 1 {
                assert_eq!(strnlen(str, len - 2), len - 2);
            }
            assert_eq!(strnlen(str, len), len);
            assert_eq!(strnlen(str, len + 1), len);
            assert_eq!(strnlen(str, len + 2), len);
            assert_eq!(strnlen(str, len + 10), len);
            buf[s + len] = b'A';
        }
    }
}

#[test]
fn string_test_strchr() {
    let test_string = b"abcdefghijkl\0";
    let empty_string = b"\0";

    for i in 0..strlen(test_string) + 1 {
        assert_eq!(
            strchr(test_string, test_string[i]),
            Some(i),
            "char:{}",
            (b'a' + i as u8) as char
        );
    }
    assert_eq!(strchr(empty_string, 0), Some(0));
    assert_eq!(strchr(empty_string, b'a'), None);
    assert_eq!(strchr(test_string, b'z'), None);
}

#[test]
fn string_test_strnchr() {
    let test_string = b"abcdefghijkl\0";
    let empty_string = b"\0";

    for i in 0..strlen(test_string) + 1 {
        for j in 0..strlen(test_string) + 2 {
            let result = strnchr(test_string, j, test_string[i]);
            if j <= i {
                assert_eq!(result, None, "i:{i} j:{j}");
            } else {
                assert_eq!(result, Some(i), "i:{i} j:{j}");
            }
        }
    }
    assert_eq!(strnchr(empty_string, 0, 0), None);
    assert_eq!(strnchr(empty_string, 1, 0), Some(0));
    assert_eq!(strnchr(empty_string, 1, b'a'), None);
    // strnchr(NULL, 0, '\0') in C.
    assert_eq!(strnchr(b"", 0, 0), None);
}

#[test]
fn string_test_strrchr() {
    let buf_size = page_align(STRING_TEST_MAX_LEN + STRING_TEST_MAX_OFFSET + 1);
    let mut buf = vec![b'A'; buf_size];
    for offset in 0..STRING_TEST_MAX_OFFSET {
        for len in 0..=STRING_TEST_MAX_LEN {
            let s = buf_size - 1 - offset - len;
            buf[s + len] = 0;
            assert_eq!(strrchr(&buf[s..], b'Z'), None);
            if len > 0 {
                assert_eq!(strrchr(&buf[s..], b'A'), Some(len - 1));
            } else {
                assert_eq!(strrchr(&buf[s..], b'A'), None);
            }
            buf[s + len] = b'A';
        }
    }
}

#[test]
fn string_test_strspn() {
    // str, accept, reject, strspn result, strcspn result
    let tests: &[(&str, &str, &str, usize, usize)] = &[
        ("foobar", "", "", 0, 6),
        ("abba", "abc", "ABBA", 4, 4),
        ("abba", "a", "b", 1, 1),
        ("", "abc", "abc", 0, 0),
    ];
    for (i, &(s, accept, reject, a, r)) in tests.iter().enumerate() {
        assert_eq!(strspn(s.as_bytes(), accept.as_bytes()), a, "i:{i}");
        assert_eq!(strcspn(s.as_bytes(), reject.as_bytes()), r, "i:{i}");
    }
}

fn strcmp_fill_buffers(fill1: u8, fill2: u8) -> (Vec<u8>, Vec<u8>) {
    let mut b1 = vec![fill1; STRCMP_LARGE_BUF_LEN];
    let mut b2 = vec![fill2; STRCMP_LARGE_BUF_LEN];
    b1[STRCMP_LARGE_BUF_LEN - 1] = 0;
    b2[STRCMP_LARGE_BUF_LEN - 1] = 0;
    (b1, b2)
}

#[test]
fn string_test_strcmp() {
    // Equal strings.
    expect_equal!(strcmp(b"Hello, Kernel!", b"Hello, Kernel!"));
    // First string is lexicographically less than the second.
    expect_lower!(strcmp(b"Hello, KUnit!", b"Hello, Kernel!"));
    // First string is lexicographically larger than the second.
    expect_greater!(strcmp(b"Hello, Kernel!", b"Hello, KUnit!"));
    // The empty string is less than any non-empty string.
    expect_lower!(strcmp(b"", b"Non-empty string"));
    // Two empty strings are equal.
    expect_equal!(strcmp(b"", b""));
    // Strings that differ in one character.
    expect_lower!(strcmp(b"Abacaba", b"Abadaba"));
    // Strings with the same prefix.
    expect_lower!(strcmp(
        b"Just a string",
        b"Just a string and something else"
    ));
}

#[test]
fn string_test_strcmp_long_strings() {
    let (mut b1, b2) = strcmp_fill_buffers(b'B', b'B');
    expect_equal!(strcmp(&b1, &b2));
    b1[STRCMP_CHANGE_POINT] = b'A';
    expect_lower!(strcmp(&b1, &b2));
    b1[STRCMP_CHANGE_POINT] = b'C';
    expect_greater!(strcmp(&b1, &b2));
}

#[test]
fn string_test_strncmp() {
    expect_equal!(strncmp(b"Hello, KUnit!", b"Hello, KUnit!", 13));
    expect_lower!(strncmp(b"Hello, KUnit!", b"Hello, Kernel!", 13));
    // Always equal when count is 0.
    expect_equal!(strncmp(b"Hello, Kernel!", b"Hello, KUnit!", 0));
    // Equal if count is the length of the common prefix...
    expect_equal!(strncmp(b"Abacaba", b"Abadaba", 3));
    // ...and not equal one past it.
    expect_lower!(strncmp(b"Abacaba", b"Abadaba", 4));
    // A prefix of another string is smaller...
    let long = b"Just a string and something else";
    expect_lower!(strncmp(b"Just a string", long, strlen(long)));
    // ...unless only the prefix is compared.
    expect_equal!(strncmp(b"Just a string", long, strlen(b"Just a string")));
}

#[test]
fn string_test_strncmp_long_strings() {
    let (mut b1, b2) = strcmp_fill_buffers(b'B', b'B');
    expect_equal!(strncmp(&b1, &b2, STRCMP_LARGE_BUF_LEN));
    b1[STRCMP_CHANGE_POINT] = b'A';
    expect_lower!(strncmp(&b1, &b2, STRCMP_LARGE_BUF_LEN));
    b1[STRCMP_CHANGE_POINT] = b'C';
    expect_greater!(strncmp(&b1, &b2, STRCMP_LARGE_BUF_LEN));
    // Equal up to the change point.
    expect_equal!(strncmp(&b1, &b2, STRCMP_CHANGE_POINT));
    expect_greater!(strncmp(&b1, &b2, STRCMP_CHANGE_POINT + 1));
}

#[test]
fn string_test_strcasecmp() {
    // The same string in different case is equal.
    expect_equal!(strcasecmp(b"Hello, Kernel!", b"HeLLO, KErNeL!"));
    expect_equal!(strcasecmp(b"", b""));
    // 'a' has a larger code than 'B', but 'a' < 'B'.
    expect_lower!(strcasecmp(b"a", b"B"));
    expect_greater!(strcasecmp(b"B", b"a"));
    // Symbols and digits.
    expect_equal!(strcasecmp(b"-+**.1230ghTTT~^", b"-+**.1230Ghttt~^"));
}

#[test]
fn string_test_strcasecmp_long_strings() {
    let (mut b1, b2) = strcmp_fill_buffers(b'b', b'B');
    expect_equal!(strcasecmp(&b1, &b2));
    b1[STRCMP_CHANGE_POINT] = b'a';
    expect_lower!(strcasecmp(&b1, &b2));
    b1[STRCMP_CHANGE_POINT] = b'C';
    expect_greater!(strcasecmp(&b1, &b2));
}

#[test]
fn string_test_strncasecmp() {
    expect_equal!(strncasecmp(b"AbAcAbA", b"Abacaba", strlen(b"Abacaba")));
    // Only count characters are checked.
    expect_equal!(strncasecmp(b"AbaCaBa", b"abaCaDa", 5));
    expect_lower!(strncasecmp(b"a", b"B", 1));
    expect_greater!(strncasecmp(b"B", b"a", 1));
    // Always equal when count is 0.
    expect_equal!(strncasecmp(b"Abacaba", b"Not abacaba", 0));
}

#[test]
fn string_test_strncasecmp_long_strings() {
    let (mut b1, b2) = strcmp_fill_buffers(b'b', b'B');
    expect_equal!(strncasecmp(&b1, &b2, STRCMP_LARGE_BUF_LEN));
    b1[STRCMP_CHANGE_POINT] = b'a';
    expect_lower!(strncasecmp(&b1, &b2, STRCMP_LARGE_BUF_LEN));
    b1[STRCMP_CHANGE_POINT] = b'C';
    expect_greater!(strncasecmp(&b1, &b2, STRCMP_LARGE_BUF_LEN));
    expect_equal!(strncasecmp(&b1, &b2, STRCMP_CHANGE_POINT));
    expect_greater!(strncasecmp(&b1, &b2, STRCMP_CHANGE_POINT + 1));
}

/// `strscpy_check()`: runs `strscpy_pad()` into a poisoned six-byte buffer
/// and checks the return value, copied characters, terminator, padding,
/// and that the poison past them is untouched.
fn strscpy_check(
    src: &str,
    count: usize,
    expected: i32,
    chars: usize,
    terminator: usize,
    pad: usize,
) {
    const POISON: u8 = b'z';
    let src = src.as_bytes();
    let mut buf = [POISON; 6];

    // Room for the NUL and one byte to detect overflow.
    assert!(count <= buf.len() - 2, "count ({count}) is too big");
    assert!(
        expected < count as i32,
        "expected ({expected}) is bigger than possible"
    );

    let written = match sized_strscpy_pad(&mut buf, src, count) {
        Ok(n) => n as i32,
        Err(e) => e.to_errno(),
    };
    assert_eq!(written, expected, "src {src:?} count {count}");

    if count != 0 && written == E2BIG.to_errno() {
        assert_eq!(
            strncmp(&buf, src, count - 1),
            0,
            "buffer state invalid for -E2BIG"
        );
        assert_eq!(
            buf[count - 1],
            0,
            "too big string is not null terminated correctly"
        );
    }
    for i in 0..chars {
        assert_eq!(buf[i], src[i], "buf[{i}]");
    }
    if terminator != 0 {
        assert_eq!(buf[count - 1], 0, "string is not null terminated correctly");
    }
    for i in 0..pad {
        assert_eq!(
            buf[chars + terminator + i],
            0,
            "padding missing at index {i}"
        );
    }
    let nr_bytes_poison = buf.len() - chars - terminator - pad;
    for i in 0..nr_bytes_poison {
        assert_eq!(
            buf[buf.len() - 1 - i],
            POISON,
            "poison value missing at index {i}"
        );
    }
}

#[test]
fn string_test_strscpy() {
    let e2big = E2BIG.to_errno();

    // strscpy_check(src, count, expected, chars, terminator, pad)
    strscpy_check("a", 0, e2big, 0, 0, 0);
    strscpy_check("", 0, e2big, 0, 0, 0);

    strscpy_check("a", 1, e2big, 0, 1, 0);
    strscpy_check("", 1, 0, 0, 1, 0);

    strscpy_check("ab", 2, e2big, 1, 1, 0);
    strscpy_check("a", 2, 1, 1, 1, 0);
    strscpy_check("", 2, 0, 0, 1, 1);

    strscpy_check("abc", 3, e2big, 2, 1, 0);
    strscpy_check("ab", 3, 2, 2, 1, 0);
    strscpy_check("a", 3, 1, 1, 1, 1);
    strscpy_check("", 3, 0, 0, 1, 2);

    strscpy_check("abcd", 4, e2big, 3, 1, 0);
    strscpy_check("abc", 4, 3, 3, 1, 0);
    strscpy_check("ab", 4, 2, 2, 1, 1);
    strscpy_check("a", 4, 1, 1, 1, 2);
    strscpy_check("", 4, 0, 0, 1, 3);

    // Fixed source strings.
    let mut dest = [0u8; 8];
    assert_eq!(strscpy(&mut dest, b""), Ok(0));
    assert_eq!(sized_strscpy(&mut dest, b"", 3), Ok(0));
    assert_eq!(sized_strscpy(&mut dest, b"", 1), Ok(0));
    assert_eq!(sized_strscpy(&mut dest, b"", 0), Err(E2BIG));
    assert_eq!(strscpy(&mut dest, b"Fixed"), Ok(5));
    assert_eq!(sized_strscpy(&mut dest, b"Fixed", 3), Err(E2BIG));
    assert_eq!(sized_strscpy(&mut dest, b"Fixed", 1), Err(E2BIG));
    assert_eq!(sized_strscpy(&mut dest, b"Fixed", 0), Err(E2BIG));
    assert_eq!(strscpy(&mut dest, b"This is too long"), Err(E2BIG));
}

#[test]
fn string_test_strcat() {
    let mut dest = [0u8; 8];
    // The destination is terminated.
    assert_eq!(strlen(&dest), 0);
    // An empty copy does nothing.
    strcat(&mut dest, b"");
    assert!(streq(&dest, ""));
    // Four characters copied in, stopping at the NUL.
    strcat(&mut dest, b"four\x00123");
    assert!(streq(&dest, "four"));
    assert_eq!(dest[5], 0);
    // Two more characters fit.
    strcat(&mut dest, b"AB");
    assert!(streq(&dest, "fourAB"));
}

#[test]
fn string_test_strncat() {
    let mut dest = [0u8; 8];
    assert_eq!(strlen(&dest), 0);
    // Empty copies of size 0 and 1 do nothing.
    strncat(&mut dest, b"", 0);
    assert!(streq(&dest, ""));
    strncat(&mut dest, b"", 1);
    assert!(streq(&dest, ""));
    // A copy of at most 0 characters does nothing.
    strncat(&mut dest, b"asdf", 0);
    assert!(streq(&dest, ""));

    // Four characters copied in, even though the maximum is 8.
    strncat(&mut dest, b"four\x00123", 8);
    assert!(streq(&dest, "four"));
    assert_eq!(dest[5], 0);
    assert_eq!(dest[6], 0);
    // Two characters copied in, two ignored.
    strncat(&mut dest, b"ABCD", 2);
    assert!(streq(&dest, "fourAB"));
}

#[test]
fn string_test_strlcat() {
    let mut dest = [0u8; 8];
    let len = dest.len();

    assert_eq!(strlen(&dest), 0);
    // An empty copy has size 0.
    assert_eq!(strlcat(&mut dest, b"", len), 0);
    assert!(streq(&dest, ""));
    // Size 1 keeps the buffer terminated and reports the source size only.
    assert_eq!(strlcat(&mut dest, b"four", 1), 4);
    assert!(streq(&dest, ""));

    // Four characters copied in.
    assert_eq!(strlcat(&mut dest, b"four", len), 4);
    assert!(streq(&dest, "four"));
    // Two more, six in total.
    assert_eq!(strlcat(&mut dest, b"AB", len), 6);
    assert!(streq(&dest, "fourAB"));
    // Two ignored once the maximum size (7) is reached.
    assert_eq!(strlcat(&mut dest, b"CD", 7), 8);
    assert!(streq(&dest, "fourAB"));
    // One of two skipped; now at the true maximum size.
    assert_eq!(strlcat(&mut dest, b"EFG", len), 9);
    assert!(streq(&dest, "fourABE"));
    // Everything else is ignored.
    assert_eq!(strlcat(&mut dest, b"1234", len), 11);
    assert!(streq(&dest, "fourABE"));
}

#[test]
fn string_test_strtomem() {
    // A canary word on each side of an eight-byte non-string field.
    let input: [u8; 8] = *b"hi\0\0\0\0\0\0";
    let truncate = b"this is too long\0";
    let mut wrap = [0xffu8; 24];
    let out = 8..16;

    // An unpadded copy leaves its surroundings untouched.
    strtomem(&mut wrap[out.clone()], &input);
    assert_eq!(&wrap[..8], &[0xff; 8]);
    assert_eq!(&wrap[8..10], b"hi");
    assert_eq!(&wrap[10..16], &[0xff; 6]);
    assert_eq!(&wrap[16..], &[0xff; 8]);

    // So does a truncated copy.
    wrap.fill(0xff);
    strtomem(&mut wrap[out.clone()], truncate);
    assert_eq!(&wrap[..8], &[0xff; 8]);
    assert_eq!(&wrap[out.clone()], &truncate[..8]);
    assert_eq!(&wrap[16..], &[0xff; 8]);

    // A padded copy pads only the string field.
    wrap.fill(0xff);
    strtomem_pad(&mut wrap[out.clone()], &input, 0xaa);
    assert_eq!(&wrap[..8], &[0xff; 8]);
    assert_eq!(&wrap[8..10], b"hi");
    assert_eq!(&wrap[10..16], &[0xaa; 6]);
    assert_eq!(&wrap[16..], &[0xff; 8]);

    // A truncated padded copy has no padding.
    wrap.fill(0xff);
    strtomem_pad(&mut wrap[out.clone()], truncate, 0xaa);
    assert_eq!(&wrap[..8], &[0xff; 8]);
    assert_eq!(&wrap[out], &truncate[..8]);
    assert_eq!(&wrap[16..], &[0xff; 8]);
}

#[test]
fn string_test_memtostr() {
    let nonstring: [u8; 7] = *b"abcdefg";
    let nonstring_small: [u8; 3] = *b"abc";
    let mut dest = [0u8; 8];

    // A non-terminated string into an exactly right-sized destination.
    dest.fill(b'X');
    memtostr(&mut dest, &nonstring);
    assert!(streq(&dest, "abcdefg"));
    dest.fill(b'X');
    memtostr(&mut dest, &nonstring_small);
    assert!(streq(&dest, "abc"));
    assert_eq!(dest[7], b'X');

    dest.fill(b'X');
    memtostr_pad(&mut dest, &nonstring);
    assert!(streq(&dest, "abcdefg"));
    dest.fill(b'X');
    memtostr_pad(&mut dest, &nonstring_small);
    assert!(streq(&dest, "abc"));
    assert_eq!(dest[7], 0);
}

#[test]
fn string_test_strends() {
    assert!(strends(b"foo-bar", b"bar"));
    assert!(strends(b"foo-bar", b"-bar"));
    assert!(strends(b"foobar", b"foobar"));
    assert!(strends(b"foobar", b""));
    assert!(!strends(b"bar", b"foobar"));
    assert!(!strends(b"", b"foo"));
    assert!(!strends(b"foobar", b"ba"));
    assert!(strends(b"", b""));
}

// Not in the C test: functions it does not cover.

#[test]
fn rinux_strsep() {
    let mut s = Some(&b"a,b,,c"[..]);
    let mut tokens = Vec::new();
    while let Some(tok) = strsep(&mut s, b",") {
        tokens.push(tok);
    }
    assert_eq!(tokens, [&b"a"[..], b"b", b"", b"c"]);
    assert_eq!(s, None);
    assert_eq!(strsep(&mut s, b","), None);

    let mut s = Some(&b"key=value\0junk"[..]);
    assert_eq!(strsep(&mut s, b"=:"), Some(&b"key"[..]));
    assert_eq!(strsep(&mut s, b"=:"), Some(&b"value"[..]));
    assert_eq!(s, None);
}

#[test]
fn rinux_strstr() {
    assert_eq!(strstr(b"hello world", b"world"), Some(6));
    assert_eq!(strstr(b"hello", b""), Some(0));
    assert_eq!(strstr(b"hello", b"hello!"), None);
    assert_eq!(strstr(b"aaab", b"aab"), Some(1));
    assert_eq!(strstr(b"ab\0ab", b"b\0"), Some(1));
    assert_eq!(strnstr(b"hello world", b"world", 11), Some(6));
    assert_eq!(strnstr(b"hello world", b"world", 10), None);
    assert_eq!(strnstr(b"abc", b"", 0), Some(0));
}

#[test]
fn rinux_mem_and_char_helpers() {
    assert_eq!(memchr(b"abcabc", b'c'), Some(2));
    assert_eq!(memchr(b"abc", b'z'), None);
    assert_eq!(memchr_inv(&[7; 40], 7), None);
    let mut buf = [7u8; 40];
    buf[33] = 1;
    assert_eq!(memchr_inv(&buf, 7), Some(33));
    assert_eq!(memscan(b"abc", b'b'), 1);
    assert_eq!(memscan(b"abc", b'z'), 3);
    assert_eq!(memcmp(b"abc", b"abd", 3), -1);
    assert_eq!(memcmp(b"\xff", b"\x00", 1), 255);
    assert_eq!(memcmp(b"abc", b"abd", 2), 0);
    assert_eq!(strpbrk(b"hello", b"ol"), Some(2));
    assert_eq!(strpbrk(b"hello", b"xyz"), None);
    assert_eq!(strchrnul(b"hello", b'l'), 2);
    assert_eq!(strchrnul(b"hello", b'z'), 5);
    assert_eq!(strnchrnul(b"hello", 2, b'l'), 2);
    assert_eq!(strnchrnul(b"hello", 1, b'l'), 1);
    assert_eq!(kbasename(b"/usr/bin/busybox"), b"busybox");
    assert_eq!(kbasename(b"busybox"), b"busybox");
    assert_eq!(str_has_prefix(b"console=ttyS0", b"console="), 8);
    assert_eq!(str_has_prefix(b"root=/dev/sda", b"console="), 0);
    assert!(strstarts(b"rdinit=/init", b"rdinit="));

    let mut dest = [b'X'; 8];
    assert_eq!(stpcpy(&mut dest, b"abc"), 3);
    assert_eq!(&dest[..4], b"abc\0");
    strcpy(&mut dest, b"z");
    assert_eq!(&dest[..2], b"z\0");

    let mut padded = [0u8; 6];
    memcpy_and_pad(&mut padded, b"abcdefgh", 3, b'-');
    assert_eq!(&padded, b"abc---");
    memcpy_and_pad(&mut padded, b"abcdefgh", 8, b'-');
    assert_eq!(&padded, b"abcdef");
}

#[test]
#[should_panic(expected = "BUG_ON")]
fn rinux_strlcat_bug_on_overlong_dest() {
    let mut dest = *b"abcdef\0\0";
    strlcat(&mut dest, b"x", 4);
}

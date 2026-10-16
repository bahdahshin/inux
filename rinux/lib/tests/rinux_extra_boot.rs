// SPDX-License-Identifier: GPL-2.0
//! Tests for step 2 code that has no KUnit test in Linux: bitops, hash,
//! parser, the `bitmap.c` mappings, the find_bit variants and the parts of
//! vsprintf that the printf and scanf tests leave out.

use rinux_lib::bitmap::*;
use rinux_lib::bitops::*;
use rinux_lib::errno;
use rinux_lib::find_bit::*;
use rinux_lib::hash::*;
use rinux_lib::parser::*;
use rinux_lib::vsprintf::{Arg, num_to_str, vscnprintf, vsnprintf};
use rinux_lib::{scnprintf, snprintf, sprintf, sscanf};

#[test]
fn bitops_words() {
    assert_eq!(genmask(7, 4), 0xf0);
    assert_eq!(genmask(63, 0), u64::MAX);
    assert_eq!(genmask_u32(31, 31), 0x8000_0000);
    assert_eq!(bits_to_longs(0), 0);
    assert_eq!(bits_to_longs(65), 2);
    assert_eq!(bits_to_u32(33), 2);
    assert_eq!(ffs(0), 0);
    assert_eq!(ffs(0x18), 4);
    assert_eq!(fls(0), 0);
    assert_eq!(fls(0x18), 5);
    assert_eq!(fls64(1 << 40), 41);
    assert_eq!(__ffs(0x18), 3);
    assert_eq!(__fls(0x18), 4);
    assert_eq!(ffz(0b0111), 3);
    assert_eq!(fns(0b1011_0100, 0), 2);
    assert_eq!(fns(0b1011_0100, 2), 5);
    assert_eq!(fns(0b1011_0100, 4), 64);
    assert_eq!(get_count_order(0), -1);
    assert_eq!(get_count_order(1), 0);
    assert_eq!(get_count_order(5), 3);
    assert_eq!(get_count_order_long(1 << 40), 40);
    assert_eq!(parity8(0b0111), 1);
    assert_eq!(parity8(0b0110), 0);
    assert_eq!(sign_extend32(0x80, 7), -128);
    assert_eq!(sign_extend64(0x7f, 7), 127);
    assert_eq!(rol32(0x8000_0001, 1), 3);
    assert_eq!(ror64(1, 1), 1 << 63);
    assert_eq!(rol8(0x81, 9), 0x03);

    let mut map = [0u64; 2];
    assert!(!test_and_set_bit(70, &mut map));
    assert!(test_and_set_bit(70, &mut map));
    assert!(test_bit(70, &map));
    assert!(test_and_change_bit(70, &mut map));
    assert!(!test_and_clear_bit(70, &mut map));
    assert_eq!(map, [0, 0]);
}

#[test]
fn hash_values() {
    assert_eq!(hash_32(0, 32), 0);
    assert_eq!(hash_32(1, 32), GOLDEN_RATIO_32);
    assert_eq!(hash_64(1, 32), (GOLDEN_RATIO_64 >> 32) as u32);
    assert_eq!(hash_long(1, 8), (GOLDEN_RATIO_64 >> 56) as u32);
    for bits in 1..32 {
        assert!(hash_64(0x1234_5678_9abc_def0, bits) < 1 << bits);
    }
    let p = 0x1234_5678_9abc_def0usize as *const u8;
    assert_eq!(hash32_ptr(p), 0x1234_5678 ^ 0x9abc_def0);
    assert_eq!(hash_ptr(p, 16), hash_64(0x1234_5678_9abc_def0, 16));
}

/// The fold + onto table from the `bitmap_onto()` comment in lib/bitmap.c.
#[test]
fn bitmap_fold_onto_table() {
    let nbits = 256;
    let mut relmap = [0u64; 4];
    for b in [40, 41, 42, 43, 45, 48, 53, 61, 74, 95] {
        set_bit(b, &mut relmap);
    }
    let w = bitmap_weight(&relmap, nbits);
    let table: &[(&[usize], &[usize], &[usize])] = &[
        (&[0], &[0], &[40]),
        (&[1], &[1], &[41]),
        (&[9], &[9], &[95]),
        (&[10], &[0], &[40]),
        (&[1, 3, 5, 7], &[1, 3, 5, 7], &[41, 43, 48, 61]),
        (&[0, 1, 2, 3, 4], &[0, 1, 2, 3, 4], &[40, 41, 42, 43, 45]),
        // C lists tmp as "0 9 8 7"; the set bits in order are 0, 7, 8, 9.
        (&[0, 9, 18, 27], &[0, 7, 8, 9], &[40, 61, 74, 95]),
        (&[0, 10, 20, 30], &[0], &[40]),
        (&[0, 11, 22, 33], &[0, 1, 2, 3], &[40, 41, 42, 43]),
        (&[0, 12, 24, 36], &[0, 2, 4, 6], &[40, 42, 45, 53]),
        (&[78, 102, 211], &[1, 2, 8], &[41, 42, 74]),
    ];
    for &(orig_bits, tmp_bits, dst_bits) in table {
        let mut orig = [0u64; 4];
        for &b in orig_bits {
            set_bit(b, &mut orig);
        }
        let mut tmp = [0u64; 4];
        let mut dst = [0u64; 4];
        bitmap_fold(&mut tmp, &orig, w, nbits);
        bitmap_onto(&mut dst, &tmp, &relmap, nbits);
        assert_eq!(
            for_each_set_bit(&tmp, nbits).collect::<Vec<_>>(),
            tmp_bits,
            "orig {orig_bits:?}"
        );
        assert_eq!(
            for_each_set_bit(&dst, nbits).collect::<Vec<_>>(),
            dst_bits,
            "orig {orig_bits:?}"
        );
    }
}

/// The examples from the `bitmap_remap()` and `bitmap_bitremap()` comments.
#[test]
fn bitmap_remap_examples() {
    let mut old = [0u64; 1];
    let mut new = [0u64; 1];
    bitmap_set(&mut old, 4, 4);
    bitmap_set(&mut new, 12, 4);

    let mut src = [0u64; 1];
    for b in [1, 5, 7] {
        set_bit(b, &mut src);
    }
    let mut dst = [!0u64; 1];
    bitmap_remap(&mut dst, &src, &old, &new, 64);
    assert_eq!(for_each_set_bit(&dst, 64).collect::<Vec<_>>(), [1, 13, 15]);

    assert_eq!(bitmap_bitremap(5, &old, &new, 64), 13);
    assert_eq!(bitmap_bitremap(3, &old, &new, 64), 3);
    // An empty new map is the identity.
    assert_eq!(bitmap_bitremap(5, &old, &[0], 64), 5);
}

#[test]
fn bitmap_logic() {
    let a = [0b1100u64, 1 << 63];
    let b = [0b1010u64, 0];
    let mut d = [0u64; 2];

    assert!(bitmap_and(&mut d, &a, &b, 128));
    assert_eq!(d, [0b1000, 0]);
    bitmap_or(&mut d, &a, &b, 128);
    assert_eq!(d, [0b1110, 1 << 63]);
    bitmap_xor(&mut d, &a, &b, 128);
    assert_eq!(d, [0b0110, 1 << 63]);
    assert!(bitmap_andnot(&mut d, &a, &b, 128));
    assert_eq!(d, [0b0100, 1 << 63]);
    // The top bit is beyond nbits and does not count.
    assert!(!bitmap_andnot(&mut d, &[0, 1 << 63], &b, 127));
    assert_eq!(bitmap_weighted_or(&mut d, &a, &b, 128), 4);
    assert_eq!(bitmap_weighted_xor(&mut d, &a, &b, 128), 3);
    assert!(bitmap_intersects(&a, &b, 4));
    assert!(!bitmap_intersects(&a, &b, 3));
    assert!(bitmap_subset(&[0b1000, 0], &a, 128));
    assert!(!bitmap_subset(&b, &a, 128));
    assert!(bitmap_or_equal(&a, &b, &[0b1110, 1 << 63], 128));
    assert_eq!(bitmap_weight_and(&a, &b, 128), 1);
    assert_eq!(bitmap_weight_andnot(&a, &b, 128), 2);
    assert!(bitmap_empty(&[0, 1 << 63], 127));
    assert!(bitmap_full(&[!0, !0 >> 1], 127));

    let src = [u64::MAX, 0x1];
    let mut d = [0u64; 2];
    bitmap_shift_left(&mut d, &src, 4, 128);
    assert_eq!(d, [!0xf, 0x1f]);
    bitmap_shift_right(&mut d, &src, 4, 65);
    assert_eq!(d, [(1 << 60) | (!0 >> 4), 0]);
    bitmap_shift_right(&mut d, &src, 64, 128);
    assert_eq!(d, [1, 0]);

    let mut d = [0u64; 3];
    bitmap_copy_and_extend(&mut d, &[!0, !0, !0], 70, 192);
    assert_eq!(d, [!0, 0x3f, 0]);

    let mut d = [0u64; 1];
    bitmap_from_u64(&mut d, 0xabcd);
    assert_eq!(d, [0xabcd]);
    bitmap_set_value8(&mut d, 0x5a, 4);
    assert_eq!(bitmap_get_value8(&d, 4), 0x5a);

    let map = [0b1111_0000_1111u64];
    assert_eq!(bitmap_find_next_zero_area(&map, 64, 0, 4, 0), 4);
    assert_eq!(bitmap_find_next_zero_area(&map, 64, 0, 5, 0), 12);
}

#[test]
fn find_bit_variants() {
    let a = [0b0110_1000u64, 1];
    let b = [0b0101_0000u64, 0];
    let c = [0b0100_0000u64, 1];

    assert_eq!(find_first_and_bit(&a, &b, 128), 6);
    assert_eq!(find_first_andnot_bit(&a, &b, 128), 3);
    assert_eq!(find_first_and_and_bit(&a, &b, &c, 128), 6);
    assert_eq!(find_next_and_bit(&a, &b, 128, 7), 128);
    assert_eq!(find_next_andnot_bit(&a, &b, 128, 6), 64);
    assert_eq!(find_next_or_bit(&a, &b, 128, 7), 64);
    assert_eq!(find_nth_and_bit(&a, &c, 128, 1), 64);
    assert_eq!(find_nth_and_andnot_bit(&a, &c, &b, 128, 0), 64);
    assert_eq!(find_last_bit(&a, 128), 64);
    assert_eq!(find_last_bit(&a, 64), 6);
    assert_eq!(find_last_bit(&[0, 0], 128), 128);
    assert_eq!(find_next_and_bit_wrap(&a, &b, 128, 7), 6);
    assert_eq!(find_next_bit_wrap(&a, 65, 65), 3);
    assert_eq!(for_each_and_bit(&a, &c, 128).collect::<Vec<_>>(), [6, 64]);
    assert_eq!(
        for_each_andnot_bit(&a, &b, 128).collect::<Vec<_>>(),
        [3, 5, 64]
    );
    assert_eq!(
        for_each_or_bit(&a, &b, 128).collect::<Vec<_>>(),
        [3, 4, 5, 6, 64]
    );
    assert_eq!(find_first_zero_bit_le(&[!0u64, 0], 128), 64);
    assert_eq!(find_next_bit_le(&a, 128, 4), 5);
    assert_eq!(find_next_zero_bit_le(&a, 128, 5), 7);
}

#[test]
fn parser_match_token() {
    const OPT_UID: i32 = 1;
    const OPT_MODE: i32 = 2;
    const OPT_NAME: i32 = 3;
    const OPT_RANGE: i32 = 4;
    const OPT_PCT: i32 = 5;
    const OPT_ERR: i32 = 6;
    let table = [
        MatchToken {
            token: OPT_UID,
            pattern: Some(b"uid=%u"),
        },
        MatchToken {
            token: OPT_MODE,
            pattern: Some(b"mode=%o"),
        },
        MatchToken {
            token: OPT_NAME,
            pattern: Some(b"name=%3s"),
        },
        MatchToken {
            token: OPT_RANGE,
            pattern: Some(b"range=%d-%x"),
        },
        MatchToken {
            token: OPT_PCT,
            pattern: Some(b"pct=%d%%"),
        },
        MatchToken {
            token: OPT_ERR,
            pattern: None,
        },
    ];
    let mut args: [&[u8]; MAX_OPT_ARGS] = [&[]; MAX_OPT_ARGS];

    assert_eq!(match_token(b"uid=1000", &table, &mut args), OPT_UID);
    assert_eq!(args[0], b"1000");
    assert_eq!(match_int(args[0]), Ok(1000));
    assert_eq!(match_uint(args[0]), Ok(1000));
    assert_eq!(match_u64(args[0]), Ok(1000));

    assert_eq!(match_token(b"mode=0755", &table, &mut args), OPT_MODE);
    assert_eq!(match_octal(args[0]), Ok(0o755));

    // %3s takes at most three bytes; the rest must match what follows.
    assert_eq!(match_token(b"name=abcdef", &table, &mut args), OPT_ERR);
    assert_eq!(match_token(b"name=abc", &table, &mut args), OPT_NAME);
    assert_eq!(args[0], b"abc");

    assert_eq!(match_token(b"range=-5-ff", &table, &mut args), OPT_RANGE);
    assert_eq!(match_int(args[0]), Ok(-5));
    assert_eq!(match_hex(args[1]), Ok(0xff));

    assert_eq!(match_token(b"pct=50%", &table, &mut args), OPT_PCT);
    assert_eq!(match_token(b"pct=50", &table, &mut args), OPT_ERR);
    assert_eq!(match_token(b"uid=", &table, &mut args), OPT_ERR);
    assert_eq!(match_token(b"bogus", &table, &mut args), OPT_ERR);

    assert_eq!(match_int(b"99999999999"), Err(errno::ERANGE));
    assert_eq!(match_int(b"x"), Err(errno::EINVAL));
    assert_eq!(match_int(b"1234567890123456789012345"), Err(errno::ERANGE));
    assert_eq!(match_uint(b"-1"), Err(errno::EINVAL));
    assert_eq!(match_u64(b"0x10"), Ok(16));

    let mut buf = [0xffu8; 4];
    assert_eq!(match_strlcpy(&mut buf, b"hello"), 5);
    assert_eq!(&buf, b"hel\0");
}

#[test]
fn parser_match_wildcard() {
    let cases: &[(&[u8], &[u8], bool)] = &[
        (b"*", b"", true),
        (b"*", b"anything", true),
        (b"a*c", b"abbbc", true),
        (b"a*c", b"abbb", false),
        (b"a?c", b"abc", true),
        (b"a?c", b"ac", false),
        (b"*.ko", b"ext4.ko", true),
        (b"*.ko", b"ext4.ko.xz", false),
        (b"x*y*z", b"xaaybbz", true),
        (b"abc", b"abc", true),
        (b"abc", b"abd", false),
        (b"ab**", b"ab", true),
    ];
    for &(pattern, s, expected) in cases {
        assert_eq!(match_wildcard(pattern, s), expected, "{pattern:?} {s:?}");
    }
}

#[test]
fn vsprintf_extras() {
    let mut buf = [0u8; 64];

    // num_to_str()
    assert_eq!(num_to_str(&mut buf, 0, 0), 1);
    assert_eq!(&buf[..1], b"0");
    assert_eq!(num_to_str(&mut buf, 1234, 6), 6);
    assert_eq!(&buf[..6], b"  1234");
    assert_eq!(num_to_str(&mut buf[..3], 1234, 0), 0);
    assert_eq!(num_to_str(&mut buf, u64::MAX, 0), 20);

    // scnprintf() returns what it wrote, snprintf() what it wanted.
    assert_eq!(snprintf!(&mut buf[..4], "%d", 123456), 6);
    assert_eq!(scnprintf!(&mut buf[..4], "%d", 123456), 3);
    assert_eq!(&buf[..4], b"123\0");
    assert_eq!(vscnprintf(&mut [], "x", &[]), 0);
    assert_eq!(sprintf!(&mut buf, "%s-%c", "ab", b'c'), 4);

    // %pV, nested.
    let inner = [Arg::from(42), Arg::from("x")];
    let n = snprintf!(&mut buf, "[%pV]", Arg::Fmt(b"%d%s", &inner));
    assert_eq!(&buf[..n], b"[42x]");

    // %pA, Rust format arguments.
    let n = snprintf!(&mut buf, "<%pA>", &format_args!("{}+{}", 1, 2));
    assert_eq!(&buf[..n], b"<1+2>");

    // %pa, %pNF, %ps without kallsyms, %pks and %pus.
    let addr = 0x1234u64.to_ne_bytes();
    let n = snprintf!(&mut buf, "%pa|%pad|%pNF", &addr, &addr, &addr);
    assert_eq!(
        &buf[..n],
        b"0x0000000000001234|0x0000000000001234|0x0000000000001234"
    );
    let n = snprintf!(&mut buf, "%ps", Arg::Ptr(0xffffffff81000000));
    assert_eq!(&buf[..n], b"0xffffffff81000000");
    let n = snprintf!(&mut buf, "%pks|%pus|%pkx", "k", "u", "z");
    assert_eq!(&buf[..n], b"k|u|(einval)");

    // Short data and wrong kinds of arguments.
    let n = snprintf!(&mut buf, "%pM|%pU|%s|%pb", &[1u8, 2], &[0u8; 4], 5, "x");
    assert_eq!(&buf[..n], b"(einval)|(einval)|(efault)|(efault)");
    let n = snprintf!(
        &mut buf,
        "%pI4|%piS|%pIS",
        &[10u8, 0, 0, 1],
        &[0u8; 16],
        &[0u8; 16]
    );
    assert_eq!(&buf[..n], b"10.0.0.1|(%pi?)|(%pI?)");

    // IPv4-mapped IPv6 addresses print the IPv4 part dotted.
    let mapped = [0u8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0xff, 0xff, 192, 168, 1, 2];
    let n = snprintf!(&mut buf, "%pI6c", &mapped);
    assert_eq!(&buf[..n], b"::ffff:192.168.1.2");
    let zeros = [0u8; 16];
    let n = snprintf!(&mut buf, "%pI6c", &zeros);
    assert_eq!(&buf[..n], b"::");

    // %pI4h and %pi4l read the bytes little-endian.
    let n = snprintf!(&mut buf, "%pI4h|%pi4l", &[1u8, 2, 3, 4], &[1u8, 2, 3, 4]);
    assert_eq!(&buf[..n], b"4.3.2.1|004.003.002.001");

    // A missing argument ends formatting; so does an unknown conversion.
    let n = vsnprintf(&mut buf, "a%db%dc", &[Arg::from(1)]);
    assert_eq!(&buf[..n], b"a1b");
    let n = vsnprintf(&mut buf, "a%nb%d", &[Arg::from(1), Arg::from(2)]);
    assert_eq!(&buf[..n], b"a");

    // Flags and lengths.
    let n = snprintf!(
        &mut buf,
        "%#o|%#x|%#X|%-5d|%05d|%+.3d|%lld|%zu",
        8,
        255,
        255,
        7,
        -7,
        7,
        -1i64,
        usize::MAX
    );
    assert_eq!(
        &buf[..n],
        b"010|0xff|0XFF|7    |-0007|+007|-1|18446744073709551615"
    );
    let n = snprintf!(&mut buf, "%3c|%-3c|%5.2s|%%", 'a', 'b', "xyz");
    assert_eq!(&buf[..n], b"  a|b  |   xy|%");
}

#[test]
fn sscanf_strings() {
    let mut a = [0u8; 8];
    let mut b = [0u8; 8];
    let mut c = [0u8; 8];
    let mut n = 0i32;
    let mut x = 0u32;

    assert_eq!(
        sscanf!("00:0a:95", "%2[^:]:%2[^:]:%2[^:]", &mut a, &mut b, &mut c),
        3
    );
    assert_eq!(
        (&a[..3], &b[..3], &c[..3]),
        (&b"00\0"[..], &b"0a\0"[..], &b"95\0"[..])
    );

    assert_eq!(sscanf!("  word rest", "%s%n", &mut a, &mut n), 1);
    assert_eq!(&a[..5], b"word\0");
    assert_eq!(n, 6);

    assert_eq!(sscanf!("xyz", "%2c", &mut a), 1);
    assert_eq!(&a[..2], b"xy");

    assert_eq!(sscanf!("abc 12", "%*s %u", &mut x), 1);
    assert_eq!(x, 12);

    assert_eq!(sscanf!("100%", "%u%%", &mut x), 1);
    assert_eq!(sscanf!("5", "%3[abc]", &mut a), 0);
    // A field width is required.
    assert_eq!(sscanf!("ab", "%[ab]", &mut a), 0);
    // An integer conversion needs an integer destination.
    assert_eq!(sscanf!("12", "%d", &mut a), 0);
}

#[test]
fn bits_typed() {
    assert_eq!(bit_u8(7), 0x80);
    assert_eq!(genmask_u8(7, 0), 0xff);
    assert_eq!(genmask_u16(15, 8), 0xff00);
    assert_eq!(genmask_u64(40, 40), 1 << 40);
    assert_eq!(genmask_u128(127, 64), u128::MAX << 64);
    assert_eq!(bit_u128(100), 1 << 100);
    assert_eq!(bit_ull_mask(65), 2);
    assert_eq!(bit_ull_word(65), 1);
    assert_eq!(bits_per_type::<u16>(), 16);
}

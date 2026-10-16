// SPDX-License-Identifier: GPL-2.0-only
//! Ports of `lib/math/tests/{gcd,int_log,int_pow,int_sqrt,rational}_kunit.c`,
//! plus tests for lcm, reciprocal_div and the math64 helpers, which have no
//! C tests.

use rinux_lib::bug::warn_count;
use rinux_lib::math::*;

#[test]
fn gcd_test() {
    let params: &[(u64, u64, u64, &str)] = &[
        (48, 18, 6, "GCD of 48 and 18"),
        (18, 48, 6, "GCD of 18 and 48"),
        (56, 98, 14, "GCD of 56 and 98"),
        (17, 13, 1, "Coprime numbers"),
        (101, 103, 1, "Coprime numbers"),
        (270, 192, 6, "GCD of 270 and 192"),
        (0, 5, 5, "GCD with zero"),
        (7, 0, 7, "GCD with zero reversed"),
        (36, 36, 36, "GCD of identical numbers"),
        (u64::MAX, 1, 1, "GCD of max ulong and 1"),
        (u64::MAX, u64::MAX, u64::MAX, "GCD of max ulong values"),
    ];
    for &(a, b, expected, name) in params {
        assert_eq!(gcd(a, b), expected, "{name}");
    }
}

#[test]
fn intlog2_test() {
    // The expected results include the log error.
    let params: &[(u32, u32, &str)] = &[
        (0, 0, "Log base 2 of 0"),
        (1, 0, "Log base 2 of 1"),
        (2, 16_777_216, "Log base 2 of 2"),
        (3, 26_591_232, "Log base 2 of 3"),
        (4, 33_554_432, "Log base 2 of 4"),
        (8, 50_331_648, "Log base 2 of 8"),
        (16, 67_108_864, "Log base 2 of 16"),
        (32, 83_886_080, "Log base 2 of 32"),
        (u32::MAX, 536_870_911, "Log base 2 of MAX"),
    ];
    for &(value, expected, name) in params {
        assert_eq!(intlog2(value), expected, "{name}");
    }
}

#[test]
fn intlog10_test() {
    let params: &[(u32, u32, &str)] = &[
        (0, 0, "Log base 10 of 0"),
        (1, 0, "Log base 10 of 1"),
        (6, 13_055_203, "Log base 10 of 6"),
        (10, 16_777_225, "Log base 10 of 10"),
        (100, 33_554_450, "Log base 10 of 100"),
        (1000, 50_331_675, "Log base 10 of 1000"),
        (10000, 67_108_862, "Log base 10 of 10000"),
        (u32::MAX, 161_614_247, "Log base 10 of MAX"),
    ];
    for &(value, expected, name) in params {
        assert_eq!(intlog10(value), expected, "{name}");
    }
}

#[test]
fn intlog_of_zero_warns() {
    let before = warn_count();
    assert_eq!(intlog2(0), 0);
    assert_eq!(intlog10(0), 0);
    assert!(warn_count() >= before + 2);
}

#[test]
fn int_pow_test() {
    let params: &[(u64, u32, u64, &str)] = &[
        (64, 0, 1, "Power of zero"),
        (64, 1, 64, "Power of one"),
        (0, 5, 0, "Base zero"),
        (1, 64, 1, "Base one"),
        (2, 2, 4, "Two squared"),
        (2, 3, 8, "Two cubed"),
        (5, 5, 3125, "Five raised to the fifth power"),
        (u64::MAX, 1, u64::MAX, "Max base"),
        (2, 63, 9_223_372_036_854_775_808, "Large result"),
    ];
    for &(base, exp, expected, name) in params {
        assert_eq!(int_pow(base, exp), expected, "{name}");
    }
}

#[test]
fn int_sqrt_test() {
    let params: &[(u64, u64, &str)] = &[
        (0, 0, "edge case: square root of 0"),
        (1, 1, "perfect square: square root of 1"),
        (2, 1, "non-perfect square: square root of 2"),
        (3, 1, "non-perfect square: square root of 3"),
        (4, 2, "perfect square: square root of 4"),
        (5, 2, "non-perfect square: square root of 5"),
        (6, 2, "non-perfect square: square root of 6"),
        (7, 2, "non-perfect square: square root of 7"),
        (8, 2, "non-perfect square: square root of 8"),
        (9, 3, "perfect square: square root of 9"),
        (15, 3, "non-perfect square: square root of 15 (N-1 from 16)"),
        (16, 4, "perfect square: square root of 16"),
        (17, 4, "non-perfect square: square root of 17 (N+1 from 16)"),
        (80, 8, "non-perfect square: square root of 80 (N-1 from 81)"),
        (81, 9, "perfect square: square root of 81"),
        (82, 9, "non-perfect square: square root of 82 (N+1 from 81)"),
        (
            255,
            15,
            "non-perfect square: square root of 255 (N-1 from 256)",
        ),
        (256, 16, "perfect square: square root of 256"),
        (
            257,
            16,
            "non-perfect square: square root of 257 (N+1 from 256)",
        ),
        (
            2_147_483_648,
            46340,
            "large input: square root of 2147483648",
        ),
        (4_294_967_295, 65535, "edge case: ULONG_MAX for 32-bit"),
    ];
    for &(x, expected, name) in params {
        assert_eq!(int_sqrt(x), expected, "{name}");
    }
}

#[test]
fn int_sqrt_is_floor_sqrt() {
    let sq = u32::MAX as u64 * u32::MAX as u64;
    for &x in &[
        u64::MAX,
        u64::MAX - 1,
        1 << 62,
        sq,
        sq - 1,
        sq + 1,
        999_999_999_999,
    ] {
        let y = int_sqrt(x);
        assert!(y * y <= x, "{x}");
        assert!((y + 1).checked_mul(y + 1).is_none_or(|sq| sq > x), "{x}");
    }
    assert_eq!(int_sqrt64(u64::MAX), u32::MAX);
}

#[test]
fn rational_test() {
    let params: &[(u64, u64, u64, u64, u64, u64, &str)] = &[
        (
            1230,
            10,
            100,
            20,
            100,
            1,
            "Exceeds bounds, semi-convergent term > 1/2 last term",
        ),
        (
            34567,
            100,
            120,
            20,
            120,
            1,
            "Exceeds bounds, semi-convergent term < 1/2 last term",
        ),
        (1, 30, 100, 10, 0, 1, "Closest to zero"),
        (1, 19, 100, 10, 1, 10, "Closest to smallest non-zero"),
        (27, 32, 16, 16, 11, 13, "Use convergent"),
        (1155, 7735, 255, 255, 33, 221, "Exact answer"),
        (87, 32, 70, 32, 68, 25, "Semiconvergent, numerator limit"),
        (
            14533,
            4626,
            15000,
            2400,
            7433,
            2366,
            "Semiconvergent, denominator limit",
        ),
    ];
    for &(num, den, max_num, max_den, exp_num, exp_den, name) in params {
        assert_eq!(
            rational_best_approximation(num, den, max_num, max_den),
            (exp_num, exp_den),
            "{name}"
        );
    }
}

#[test]
fn lcm_values() {
    assert_eq!(lcm(4, 6), 12);
    assert_eq!(lcm(21, 6), 42);
    assert_eq!(lcm(0, 6), 0);
    assert_eq!(lcm(6, 0), 0);
    assert_eq!(lcm_not_zero(0, 6), 6);
    assert_eq!(lcm_not_zero(6, 0), 6);
    assert_eq!(lcm_not_zero(0, 0), 0);
    assert_eq!(lcm_not_zero(4, 6), 12);
}

#[test]
fn reciprocal_divide_matches_division() {
    let divisors = [
        1u32,
        2,
        3,
        5,
        7,
        10,
        60,
        1000,
        4096,
        65535,
        0x7fff_ffff,
        0x8000_0000,
        0xffff_fffe,
        u32::MAX,
    ];
    let dividends = [
        0u32,
        1,
        2,
        3,
        59,
        60,
        61,
        999,
        1000,
        4097,
        0x1234_5678,
        0x7fff_ffff,
        0x8000_0000,
        u32::MAX,
    ];
    for &d in &divisors {
        let r = reciprocal_value(d);
        for &a in &dividends {
            assert_eq!(reciprocal_divide(a, r), a / d, "{a} / {d}");
        }
    }
}

#[test]
fn reciprocal_value_adv_examples() {
    // Division by 7 needs the 33-bit multiplier; by 3, a narrow one.
    let r7 = reciprocal_value_adv(7, 32);
    assert_eq!(
        (r7.m, r7.sh, r7.exp, r7.is_wide_m),
        (0x2492_4925, 3, 3, true)
    );
    let r3 = reciprocal_value_adv(3, 32);
    assert_eq!(
        (r3.m, r3.sh, r3.exp, r3.is_wide_m),
        (0xaaaa_aaab, 1, 2, false)
    );
    for a in [0u32, 1, 2, 3, 4, 100, 0x7fff_ffff, u32::MAX] {
        assert_eq!(((a as u64 * r3.m as u64) >> 32 >> r3.sh) as u32, a / 3);
    }
}

#[test]
fn math64_helpers() {
    assert_eq!(div_u64_rem(100, 7), (14, 2));
    assert_eq!(div_s64_rem(-100, 7), (-14, -2));
    assert_eq!(
        div64_u64_rem(u64::MAX, 1 << 40),
        (u64::MAX >> 40, (1 << 40) - 1)
    );
    assert_eq!(div64_s64(-9, 2), -4);
    let mut n = 1_000_000_007u64;
    assert_eq!(do_div(&mut n, 10), 7);
    assert_eq!(n, 100_000_000);
    assert_eq!(iter_div_u64_rem(1_000_000_123, 1_000_000_000), (1, 123));
    assert_eq!(iter_div_u64_rem(5, 10), (0, 5));
    // (2^64 - 1)(2^32 - 1) >> 32 == 2^64 - 2^32 - 1
    assert_eq!(
        mul_u64_u32_shr(u64::MAX, u32::MAX, 32),
        u64::MAX - (1 << 32)
    );
    assert_eq!(mul_u64_u64_shr(1 << 63, 4, 64), 2);
    assert_eq!(mul_s64_u64_shr(-6, 4, 1), (-12i64) as u64);
    assert_eq!(div64_u64_round_up(10, 3), 4);
    assert_eq!(div_u64_round_up(9, 3), 3);
    assert_eq!(div64_u64_round_closest(10, 4), 3);
    assert_eq!(div_u64_round_closest(9, 4), 2);
    assert_eq!(div_s64_round_closest(-10, 4), -3);
    assert_eq!(div_s64_round_closest(-9, 4), -2);
    assert_eq!(div_s64_round_closest(10, -4), -3);
}

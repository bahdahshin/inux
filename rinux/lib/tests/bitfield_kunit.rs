// SPDX-License-Identifier: GPL-2.0+
//! Port of `lib/tests/bitfield_kunit.c`: every `CHECK_ENC_GET` and `CHECK`
//! case, plus the `FIELD_*` macros, which the C test leaves to the compiler.
//!
//! The `TEST_BITFIELD_COMPILE` cases are build failures in C and are not
//! ported; the bad-mask panic is checked instead.

use rinux_lib::bitfield::*;

/// `CHECK_ENC_GET_U()`: encode, compare, and get back.
macro_rules! check_enc_get_u {
    ($enc:ident, $get:ident, $v:expr, $field:expr, $res:expr) => {{
        let res = $enc($v, $field);
        assert_eq!(
            res,
            $res,
            "{}({:#x}, {:#x}) is {:#x} != {:#x}",
            stringify!($enc),
            $v,
            $field,
            res,
            $res
        );
        assert_eq!($get(res, $field), $v);
    }};
}

/// `CHECK_ENC_GET_LE()` / `CHECK_ENC_GET_BE()`: the result is compared in
/// stored form, `cpu_to_le##tp(res)`.
macro_rules! check_enc_get_endian {
    ($enc:ident, $get:ident, $to:expr, $v:expr, $field:expr, $res:expr) => {{
        let res = $enc($v, $field);
        assert_eq!(
            res,
            $to($res),
            "{}({:#x}, {:#x}) is {:#x} != {:#x}",
            stringify!($enc),
            $v,
            $field,
            res,
            $to($res)
        );
        assert_eq!($get(res, $field), $v);
    }};
}

/// `CHECK_ENC_GET()`: the native, little-endian and big-endian forms.
macro_rules! check_enc_get {
    ($t:ty, $u:ident, $ug:ident, $le:ident, $leg:ident, $be:ident, $beg:ident, $v:expr, $field:expr, $res:expr) => {{
        check_enc_get_u!($u, $ug, $v as $t, $field as $t, $res as $t);
        check_enc_get_endian!($le, $leg, <$t>::to_le, $v as $t, $field as $t, $res as $t);
        check_enc_get_endian!($be, $beg, <$t>::to_be, $v as $t, $field as $t, $res as $t);
    }};
}

macro_rules! check16 {
    ($v:expr, $field:expr, $res:expr) => {
        check_enc_get!(
            u16,
            u16_encode_bits,
            u16_get_bits,
            le16_encode_bits,
            le16_get_bits,
            be16_encode_bits,
            be16_get_bits,
            $v,
            $field,
            $res
        )
    };
}
macro_rules! check32 {
    ($v:expr, $field:expr, $res:expr) => {
        check_enc_get!(
            u32,
            u32_encode_bits,
            u32_get_bits,
            le32_encode_bits,
            le32_get_bits,
            be32_encode_bits,
            be32_get_bits,
            $v,
            $field,
            $res
        )
    };
}
macro_rules! check64 {
    ($v:expr, $field:expr, $res:expr) => {
        check_enc_get!(
            u64,
            u64_encode_bits,
            u64_get_bits,
            le64_encode_bits,
            le64_get_bits,
            be64_encode_bits,
            be64_get_bits,
            $v,
            $field,
            $res
        )
    };
}

/// `test_bitfields_constants()`.
#[test]
fn test_bitfields_constants() {
    check16!(1, 0x000f, 0x0001);
    check16!(3, 0x00f0, 0x0030);
    check16!(5, 0x0f00, 0x0500);
    check16!(7, 0xf000, 0x7000);
    check16!(14, 0x000f, 0x000e);
    check16!(15, 0x00f0, 0x00f0);

    check_enc_get_u!(u8_encode_bits, u8_get_bits, 1u8, 0x0f, 0x01);
    check_enc_get_u!(u8_encode_bits, u8_get_bits, 3u8, 0xf0, 0x30);
    check_enc_get_u!(u8_encode_bits, u8_get_bits, 14u8, 0x0f, 0x0e);
    check_enc_get_u!(u8_encode_bits, u8_get_bits, 15u8, 0xf0, 0xf0);

    check32!(1, 0x00000f00, 0x00000100);
    check32!(3, 0x0000f000, 0x00003000);
    check32!(5, 0x000f0000, 0x00050000);
    check32!(7, 0x00f00000, 0x00700000);
    check32!(14, 0x0f000000, 0x0e000000);
    check32!(15, 0xf0000000u32, 0xf0000000u32);

    check64!(1, 0x00000f0000000000u64, 0x0000010000000000u64);
    check64!(3, 0x0000f00000000000u64, 0x0000300000000000u64);
    check64!(5, 0x000f000000000000u64, 0x0005000000000000u64);
    check64!(7, 0x00f0000000000000u64, 0x0070000000000000u64);
    check64!(14, 0x0f00000000000000u64, 0x0e00000000000000u64);
    check64!(15, 0xf000000000000000u64, 0xf000000000000000u64);
}

/// `CHECK()`: every value that fits lands at the mask's shift.
macro_rules! check {
    ($t:ty, $enc:ident, $mask:expr) => {{
        let mask: $t = $mask;
        for v in 0..(1u64 << mask.count_ones()) {
            assert_eq!(
                $enc(v as $t, mask),
                (v << mask.trailing_zeros()) as $t,
                "{}({v:#x}, {mask:#x})",
                stringify!($enc)
            );
        }
    }};
}

/// `test_bitfields_variables()`.
#[test]
fn test_bitfields_variables() {
    check!(u8, u8_encode_bits, 0x0f);
    check!(u8, u8_encode_bits, 0xf0);
    check!(u8, u8_encode_bits, 0x38);
    check!(u16, u16_encode_bits, 0x0038);
    check!(u16, u16_encode_bits, 0x0380);
    check!(u16, u16_encode_bits, 0x3800);
    check!(u16, u16_encode_bits, 0x8000);
    check!(u32, u32_encode_bits, 0x80000000);
    check!(u32, u32_encode_bits, 0x7f000000);
    check!(u32, u32_encode_bits, 0x07e00000);
    check!(u32, u32_encode_bits, 0x00018000);
    check!(u64, u64_encode_bits, 0x8000000000000000);
    check!(u64, u64_encode_bits, 0x7f00000000000000);
    check!(u64, u64_encode_bits, 0x0001800000000000);
    check!(u64, u64_encode_bits, 0x0000000080000000);
    check!(u64, u64_encode_bits, 0x000000007f000000);
    check!(u64, u64_encode_bits, 0x0000000018000000);
    check!(u64, u64_encode_bits, 0x0000001f8000000);
}

/// The `FIELD_*` macros on the register layout from the header's own
/// example: fields prepared, read back, modified and sign-extended.
#[test]
fn field_macros_follow_the_header_example() {
    const REG_FIELD_A: u32 = 0x0000_003f; // bits 0..5
    const REG_FIELD_B: u32 = 0x0000_8000; // bit 15
    const REG_FIELD_C: u32 = 0x00ff_0000; // bits 16..23
    const REG_FIELD_D: u32 = 0xff00_0000; // bits 24..31

    assert_eq!(field_max(REG_FIELD_A), 0x3f);
    assert_eq!(field_max(REG_FIELD_B), 1);
    assert_eq!(field_max(REG_FIELD_C), 0xff);
    assert!(field_fit(REG_FIELD_A, 0x3f));
    assert!(!field_fit(REG_FIELD_A, 0x40));
    assert!(field_fit(REG_FIELD_D, 0xff));

    let reg = field_prep(REG_FIELD_A, 0x2a)
        | field_prep(REG_FIELD_B, 0)
        | field_prep(REG_FIELD_C, 0x80)
        | field_prep(REG_FIELD_D, 0x40);
    assert_eq!(reg, 0x4080_002a);
    assert_eq!(field_get(REG_FIELD_A, reg), 0x2a);
    assert_eq!(field_get(REG_FIELD_B, reg), 0);
    assert_eq!(field_get(REG_FIELD_C, reg), 0x80);
    assert_eq!(field_get(REG_FIELD_D, reg), 0x40);

    // An oversized value is masked, as the C run-time path does.
    assert_eq!(field_prep(REG_FIELD_A, 0x7f), 0x3f);

    let mut r = reg;
    field_modify(REG_FIELD_C, &mut r, 0x01);
    assert_eq!(r, 0x4001_002a);
    field_modify(REG_FIELD_B, &mut r, 1);
    assert_eq!(r, 0x4001_802a);

    // 0x80 in an 8-bit field is -128; 0x2a in a 6-bit field is -22.
    assert_eq!(field_get_signed(REG_FIELD_C, reg), -128);
    assert_eq!(field_get_signed(REG_FIELD_A, reg), -22);
    assert_eq!(field_get_signed(REG_FIELD_D, reg), 0x40);
    assert_eq!(
        field_get_signed(0xf000_0000_0000_0000u64, 0x8000_0000_0000_0000u64),
        -8i64
    );
}

/// `<type>_replace_bits()` and `<type>p_replace_bits()`, which the C test
/// does not cover.
#[test]
fn replace_bits_keeps_the_other_fields() {
    assert_eq!(u32_replace_bits(0xdead_beef, 0x5, 0x0000_0f00), 0xdead_b5ef);
    let mut v = 0xdead_beefu32;
    u32p_replace_bits(&mut v, 0xa, 0x0000_000f);
    assert_eq!(v, 0xdead_beea);

    // Stored forms: the field mask is byte-swapped along with the value.
    let stored = 0x1234u16.to_le();
    assert_eq!(le16_replace_bits(stored, 0xf, 0x00f0), 0x12f4u16.to_le());
    let mut s = 0x1234u16.to_be();
    be16p_replace_bits(&mut s, 0x0, 0xf000);
    assert_eq!(s, 0x0234u16.to_be());
}

/// `field_multiplier()` and `field_mask()` on the masks above, and the
/// bad-mask panic that is a build error in C.
#[test]
fn multiplier_and_mask() {
    assert_eq!(field_multiplier(0x0000_0f00), 0x100);
    assert_eq!(field_mask(0x0000_0f00), 0xf);
    assert_eq!(field_multiplier(1), 1);
    assert_eq!(field_mask(u64::MAX), u64::MAX);
    assert_eq!(
        field_multiplier(0x8000_0000_0000_0000),
        0x8000_0000_0000_0000
    );
    assert!(std::panic::catch_unwind(|| field_multiplier(0)).is_err());
    assert!(std::panic::catch_unwind(|| field_multiplier(0x0000_0f0f)).is_err());
    assert!(std::panic::catch_unwind(|| field_multiplier(0x5)).is_err());
}

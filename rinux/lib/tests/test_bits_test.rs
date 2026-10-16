// SPDX-License-Identifier: GPL-2.0+
//! Port of `lib/tests/test_bits.c`: the `BIT_*()` and `GENMASK*()` macros in
//! `include/linux/bits.h`, as the typed Rust helpers in `bitops`. The C
//! `static_assert()`s on the types are the Rust function signatures; the
//! `TEST_GENMASK_FAILURES` compile-fail cases have no runtime form and are
//! not ported. `GENMASK_INPUT_CHECK()` is a C macro with no port yet.

use rinux_lib::bitops::*;

#[test]
fn bit_helpers_set_the_named_bit() {
    assert_eq!(bit_u8(0), 1);
    assert_eq!(bit_u8(7), 0x80);
    assert_eq!(bit_u16(15), 0x8000);
    assert_eq!(bit_u32(31), 0x8000_0000);
    assert_eq!(bit_u64(63), 0x8000_0000_0000_0000);
}

#[test]
fn genmask_matches_the_c_values() {
    assert_eq!(genmask(0, 0), 1);
    assert_eq!(genmask(1, 0), 3);
    assert_eq!(genmask(2, 1), 6);
    assert_eq!(genmask(31, 0), 0xFFFF_FFFF);
}

#[test]
fn genmask_ull_matches_the_c_values() {
    assert_eq!(genmask_ull(0, 0), 1);
    assert_eq!(genmask_ull(1, 0), 3);
    assert_eq!(genmask_ull(39, 21), 0x0000_00ff_ffe0_0000);
    assert_eq!(genmask_ull(63, 0), u64::MAX);
}

#[test]
fn typed_genmask_matches_the_c_values() {
    assert_eq!(genmask_u8(0, 0), 1);
    assert_eq!(genmask_u8(7, 0), u8::MAX);
    assert_eq!(genmask_u16(1, 0), 3);
    assert_eq!(genmask_u16(15, 0), u16::MAX);
    assert_eq!(genmask_u32(16, 16), 0x1_0000);
    assert_eq!(genmask_u32(31, 0), u32::MAX);
    assert_eq!(genmask_u64(63, 0), u64::MAX);
}

#[test]
fn genmask_u128_covers_the_wide_cases() {
    // Below 64 bits.
    assert_eq!(genmask_u128(0, 0), 1);
    assert_eq!(genmask_u128(2, 1), 6);
    assert_eq!(genmask_u128(39, 21), 0x0000_00ff_ffe0_0000);
    assert_eq!(genmask_u128(63, 0), u64::MAX as u128);
    // Above 64 bits, checked by shifting the mask down to the bits it covers.
    assert_eq!(genmask_u128(64, 0) >> 1, u64::MAX as u128);
    assert_eq!(genmask_u128(81, 50) >> 50, 0xFFFF_FFFF);
    assert_eq!(genmask_u128(87, 64) >> 64, 0x00FF_FFFF);
    assert_eq!(genmask_u128(87, 80) >> 64, 0x00FF_0000);
    assert_eq!(genmask_u128(127, 0) >> 64, u64::MAX as u128);
    assert_eq!(genmask_u128(127, 0) as u64, u64::MAX);
    assert_eq!(genmask_u128(127, 126) >> 126, 3);
    assert_eq!(genmask_u128(127, 127) >> 127, 1);
}

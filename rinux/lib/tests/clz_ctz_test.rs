// SPDX-License-Identifier: GPL-2.0-or-later
//! Host tests for the `__clz*` and `__ctz*` builtins. Linux has no test
//! file for them.

use rinux_lib::clz_ctz::{clzdi2, clzsi2, ctzdi2, ctzsi2};

#[test]
fn single_bits_give_their_positions() {
    for bit in 0..32 {
        assert_eq!(ctzsi2(1 << bit), bit);
        assert_eq!(clzsi2(1 << bit), 31 - bit);
    }
    for bit in 0..64 {
        assert_eq!(ctzdi2(1 << bit), bit);
        assert_eq!(clzdi2(1 << bit), 63 - bit);
    }
}

#[test]
fn the_sign_bit_is_not_special() {
    // A negative int has its top bit set, so no leading zeros.
    assert_eq!(clzsi2(-1), 0);
    assert_eq!(clzsi2(i32::MIN), 0);
    assert_eq!(ctzsi2(i32::MIN), 31);
}

#[test]
fn zero_gives_the_bit_width() {
    assert_eq!(clzsi2(0), 32);
    assert_eq!(ctzsi2(0), 32);
    assert_eq!(clzdi2(0), 64);
    assert_eq!(ctzdi2(0), 64);
}

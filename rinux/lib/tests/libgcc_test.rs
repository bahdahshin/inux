// SPDX-License-Identifier: GPL-2.0-or-later
//! Host tests for the libgcc helpers. Linux has no KUnit file for them;
//! these check the halves algorithm against the native 64-bit operators.

use rinux_lib::libgcc::{ashldi3, ashrdi3, cmpdi2, lshrdi3, ucmpdi2};

const VALUES: [i64; 9] = [
    0,
    1,
    -1,
    0x1234_5678_9abc_def0,
    -0x1234_5678_9abc_def0,
    i64::MAX,
    i64::MIN,
    0x8000_0000,
    0xffff_ffff,
];

#[test]
fn shifts_match_the_native_operators_for_every_count() {
    for &u in &VALUES {
        for b in 0..64 {
            assert_eq!(ashldi3(u, b), u << b, "{u:#x} << {b}");
            assert_eq!(ashrdi3(u, b), u >> b, "{u:#x} >> {b}");
            assert_eq!(lshrdi3(u, b), ((u as u64) >> b) as i64, "{u:#x} >>> {b}");
        }
    }
}

#[test]
fn zero_count_returns_the_value() {
    for &u in &VALUES {
        assert_eq!(ashldi3(u, 0), u);
        assert_eq!(ashrdi3(u, 0), u);
        assert_eq!(lshrdi3(u, 0), u);
    }
}

#[test]
fn signed_compare_returns_less_equal_greater_codes() {
    assert_eq!(cmpdi2(-1, 0), 0);
    assert_eq!(cmpdi2(5, 5), 1);
    assert_eq!(cmpdi2(1, -1), 2);
    // Low halves compare unsigned: 0x1_0000_0000 is above 0xffff_ffff.
    assert_eq!(cmpdi2(0x1_0000_0000, 0xffff_ffff), 2);
}

#[test]
fn unsigned_compare_returns_less_equal_greater_codes() {
    assert_eq!(ucmpdi2(0, u64::MAX), 0);
    assert_eq!(ucmpdi2(7, 7), 1);
    assert_eq!(ucmpdi2(u64::MAX, 0), 2);
    assert_eq!(ucmpdi2(0x8000_0000, 0x7fff_ffff), 2);
}

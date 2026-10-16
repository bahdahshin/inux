// SPDX-License-Identifier: GPL-2.0-or-later
//! Host tests for `mac_pton` and the `__clz_tab` table. Linux has no KUnit
//! file for either.

use rinux_lib::clz_tab::CLZ_TAB;
use rinux_lib::net_utils::mac_pton;

#[test]
fn parses_both_cases_of_hex() {
    assert_eq!(
        mac_pton(b"00:1a:2B:3c:4D:5e"),
        Some([0x00, 0x1a, 0x2b, 0x3c, 0x4d, 0x5e])
    );
    assert_eq!(mac_pton(b"FF:FF:FF:FF:FF:FF"), Some([0xff; 6]));
}

#[test]
fn rejects_bad_separators_digits_and_lengths() {
    assert_eq!(mac_pton(b"00-1a-2b-3c-4d-5e"), None);
    assert_eq!(mac_pton(b"00:1a:2b:3c:4d:5g"), None);
    assert_eq!(mac_pton(b"00:1a:2b"), None);
    assert_eq!(mac_pton(b""), None);
}

#[test]
fn a_nul_inside_the_window_is_too_short() {
    assert_eq!(mac_pton(b"00:1a:2b:3c:4d\0\0"), None);
    // A NUL after the 17th byte is never looked at.
    assert_eq!(
        mac_pton(b"00:1a:2b:3c:4d:5e\0"),
        Some([0, 0x1a, 0x2b, 0x3c, 0x4d, 0x5e])
    );
}

#[test]
fn clz_table_gives_bit_lengths() {
    assert_eq!(CLZ_TAB[0], 0);
    assert_eq!(CLZ_TAB[1], 1);
    assert_eq!(CLZ_TAB[2], 2);
    assert_eq!(CLZ_TAB[3], 2);
    assert_eq!(CLZ_TAB[4], 3);
    assert_eq!(CLZ_TAB[7], 3);
    assert_eq!(CLZ_TAB[255], 8);
    // Each bit length 1..=8 covers 2^(n-1) entries, as the C table does.
    for n in 1..=8u8 {
        let count = CLZ_TAB.iter().filter(|&&v| v == n).count();
        assert_eq!(count, 1 << (n - 1), "bit length {n}");
    }
}

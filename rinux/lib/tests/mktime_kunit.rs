// SPDX-License-Identifier: GPL-2.0
//! Tests for `mktime64` against known Unix timestamps.

use rinux_lib::mktime::mktime64;

#[test]
fn known_epochs() {
    assert_eq!(mktime64(1970, 1, 1, 0, 0, 0), 0);
    assert_eq!(mktime64(2000, 1, 1, 0, 0, 0), 946_684_800);
    // A date just after a leap day.
    assert_eq!(mktime64(2000, 3, 1, 0, 0, 0), 951_868_800);
    assert_eq!(mktime64(2024, 2, 29, 12, 0, 0), 1_709_208_000);
    // The classic "unix billennium".
    assert_eq!(mktime64(2001, 9, 9, 1, 46, 40), 1_000_000_000);
    // A second past the epoch, and a full day.
    assert_eq!(mktime64(1970, 1, 1, 0, 0, 1), 1);
    assert_eq!(mktime64(1970, 1, 2, 0, 0, 0), 86_400);
}

#[test]
fn matches_a_reference_implementation() {
    // Days-from-civil reference (Howard Hinnant's algorithm), independent of
    // the ported formula, checked across a range of dates.
    fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
        let y = if m <= 2 { y - 1 } else { y };
        let era = if y >= 0 { y } else { y - 399 } / 400;
        let yoe = y - era * 400;
        let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        era * 146097 + doe - 719468
    }
    for &(y, mo, d, h, mi, s) in &[
        (1999u32, 12u32, 31u32, 23u32, 59u32, 59u32),
        (2026, 10, 15, 21, 38, 36),
        (2038, 1, 19, 3, 14, 7),
        (1984, 2, 29, 0, 0, 0),
    ] {
        let want = days_from_civil(y as i64, mo as i64, d as i64) * 86400
            + h as i64 * 3600
            + mi as i64 * 60
            + s as i64;
        assert_eq!(mktime64(y, mo, d, h, mi, s), want, "for {y}-{mo}-{d}");
    }
}

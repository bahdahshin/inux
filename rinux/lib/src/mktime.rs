// SPDX-License-Identifier: GPL-2.0
//! Converting a broken-down UTC date to a Unix timestamp.
//!
//! A port of `mktime64()` from `kernel/time/time.c` (originally Gauss'
//! algorithm by way of Linus): the number of seconds since 1970-01-01 for a
//! proleptic-Gregorian UTC date, with no range checks (the caller passes a
//! valid date, e.g. from the RTC).

/// `mktime64()`: seconds since the Unix epoch for the given UTC date/time.
///
/// `year` is the full year (e.g. 2026) and `mon` is 1..=12.
pub fn mktime64(year: u32, mon: u32, day: u32, hour: u32, min: u32, sec: u32) -> i64 {
    let mut mon = mon as i64 - 2; // Jan and Feb are months 13 and 14 of the
    let mut year = year as i64; // previous year, so leap days land at the end.
    if mon <= 0 {
        mon += 12;
        year -= 1;
    }
    ((((year / 4 - year / 100 + year / 400 + 367 * mon / 12 + day as i64) + year * 365 - 719499)
        * 24
        + hour as i64)
        * 60
        + min as i64)
        * 60
        + sec as i64
}

// SPDX-License-Identifier: GPL-2.0-or-later
//! Host tests for `time64_to_tm`: known dates, the offset, the leap and
//! century rules, and a round trip through `mktime64`.

use rinux_lib::mktime::mktime64;
use rinux_lib::timeconv::{Tm, time64_to_tm};

#[allow(clippy::too_many_arguments)]
fn tm(sec: i32, min: i32, hour: i32, mday: i32, mon: i32, year: i32, wday: i32, yday: i32) -> Tm {
    Tm {
        tm_sec: sec,
        tm_min: min,
        tm_hour: hour,
        tm_mday: mday,
        tm_mon: mon,
        tm_year: year,
        tm_wday: wday,
        tm_yday: yday,
    }
}

#[test]
fn the_epoch_is_a_thursday() {
    assert_eq!(time64_to_tm(0, 0), tm(0, 0, 0, 1, 0, 70, 4, 0));
}

#[test]
fn one_second_before_the_epoch() {
    assert_eq!(time64_to_tm(-1, 0), tm(59, 59, 23, 31, 11, 69, 3, 364));
}

#[test]
fn leap_day_in_a_century_that_divides_by_400() {
    // 2000-02-29 00:00:00 UTC, a Tuesday, day 59 of the year.
    assert_eq!(time64_to_tm(951_782_400, 0), tm(0, 0, 0, 29, 1, 100, 2, 59));
}

#[test]
fn a_famous_timestamp() {
    // 2009-02-13 23:31:30 UTC, a Friday, day 43 of the year.
    assert_eq!(
        time64_to_tm(1_234_567_890, 0),
        tm(30, 31, 23, 13, 1, 109, 5, 43)
    );
}

#[test]
fn offset_moves_across_midnight() {
    assert_eq!(time64_to_tm(0, 3600), tm(0, 0, 1, 1, 0, 70, 4, 0));
    assert_eq!(time64_to_tm(0, -1), tm(59, 59, 23, 31, 11, 69, 3, 364));
}

#[test]
fn no_leap_day_in_a_century_that_does_not_divide_by_400() {
    // 2100-02-28 23:59:59 is followed by 2100-03-01.
    let secs = mktime64(2100, 2, 28, 23, 59, 59);
    let feb = time64_to_tm(secs, 0);
    let mar = time64_to_tm(secs + 1, 0);
    assert_eq!((feb.tm_mon, feb.tm_mday), (1, 28));
    assert_eq!((mar.tm_mon, mar.tm_mday), (2, 1));
}

#[test]
fn round_trips_through_mktime64() {
    let mut t: i64 = -2_000_000_000;
    while t < 4_000_000_000 {
        let r = time64_to_tm(t, 0);
        let back = mktime64(
            (r.tm_year + 1900) as u32,
            (r.tm_mon + 1) as u32,
            r.tm_mday as u32,
            r.tm_hour as u32,
            r.tm_min as u32,
            r.tm_sec as u32,
        );
        assert_eq!(back, t, "for {t}");
        t += 86_400 * 37 + 12_345;
    }
}

// SPDX-License-Identifier: LGPL-2.0+
//! Converting calendar seconds to a broken-down UTC time.
//!
//! Ported from `kernel/time/timeconv.c`, `time64_to_tm()`. The algorithm is
//! Proposition 6.3 of Neri and Schneider, "Euclidean Affine Functions and
//! Applications to Calendar Algorithms" (arXiv:2102.06959): it counts days
//! in a calendar whose year starts in March, so leap days fall at the end.
//! Every step keeps C's unsigned widths and wrapping, so the results match
//! for every input the kernel can see.

/// `struct tm`, the broken-down time that [`time64_to_tm`] fills in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Tm {
    /// Seconds after the minute, 0 to 59.
    pub tm_sec: i32,
    /// Minutes after the hour, 0 to 59.
    pub tm_min: i32,
    /// Hours past midnight, 0 to 23.
    pub tm_hour: i32,
    /// Day of the month, 1 to 31.
    pub tm_mday: i32,
    /// Months since January, 0 to 11.
    pub tm_mon: i32,
    /// Years since 1900.
    pub tm_year: i32,
    /// Days since Sunday, 0 to 6.
    pub tm_wday: i32,
    /// Days since January 1, 0 to 365.
    pub tm_yday: i32,
}

const SECS_PER_HOUR: i64 = 60 * 60;
const SECS_PER_DAY: i64 = SECS_PER_HOUR * 24;

/// `time64_to_tm()`: the broken-down time of `totalsecs` seconds since the
/// Unix epoch, shifted by `offset` seconds.
pub fn time64_to_tm(totalsecs: i64, offset: i32) -> Tm {
    let mut days = totalsecs / SECS_PER_DAY;
    let mut rem = totalsecs % SECS_PER_DAY;
    rem += offset as i64;
    while rem < 0 {
        rem += SECS_PER_DAY;
        days -= 1;
    }
    while rem >= SECS_PER_DAY {
        rem -= SECS_PER_DAY;
        days += 1;
    }

    let tm_hour = rem / SECS_PER_HOUR;
    rem %= SECS_PER_HOUR;
    let tm_min = rem / 60;
    let tm_sec = rem % 60;

    // January 1, 1970 was a Thursday.
    let mut tm_wday = (4 + days) % 7;
    if tm_wday < 0 {
        tm_wday += 7;
    }

    // Days from 0000-03-01 in the computational calendar, where March is
    // month 0 of the year.
    let udays = (days as u64).wrapping_add(2_305_843_009_213_814_918);

    let mut u64tmp = udays.wrapping_mul(4).wrapping_add(3);
    let century = u64tmp / 146_097;
    u64tmp %= 146_097;
    let day_of_century = (u64tmp / 4) as u32;

    let u32tmp = day_of_century.wrapping_mul(4).wrapping_add(3);
    let u64tmp = 2_939_745u64 * u32tmp as u64;
    let year_of_century = (u64tmp >> 32) as u32;
    let day_of_year = (u64tmp as u32) / 2_939_745 / 4;

    let mut year = century
        .wrapping_mul(100)
        .wrapping_add(year_of_century as u64);
    let is_leap_year = if year_of_century != 0 {
        year_of_century % 4 == 0
    } else {
        century % 4 == 0
    };

    let u32tmp = day_of_year.wrapping_mul(2141).wrapping_add(132_377);
    let mut month = u32tmp >> 16;
    let day = ((u32tmp as u16) as u32) / 2141;

    // January 1st is day 306 of the computational year.
    let is_jan_or_feb = day_of_year >= 306;

    // Back to the Gregorian calendar, and to Unix time.
    year = year
        .wrapping_add(is_jan_or_feb as u64)
        .wrapping_sub(6_313_183_731_940_000);
    if is_jan_or_feb {
        month = month.wrapping_sub(12);
    }
    let day = day + 1;
    let day_of_year = if is_jan_or_feb {
        day_of_year.wrapping_sub(306)
    } else {
        day_of_year + 31 + 28 + is_leap_year as u32
    };

    Tm {
        tm_sec: tm_sec as i32,
        tm_min: tm_min as i32,
        tm_hour: tm_hour as i32,
        tm_mday: day as i32,
        tm_mon: month as i32,
        tm_year: year.wrapping_sub(1900) as i32,
        tm_wday: tm_wday as i32,
        tm_yday: day_of_year as i32,
    }
}

// SPDX-License-Identifier: LGPL-2.1+
//! Port of `kernel/time/time_test.c`: `time64_to_tm()` over 80000 years, one
//! day at a time, checked against a calendar that advances day by day.

use rinux_lib::timeconv::time64_to_tm;

fn is_leap(year: i64) -> bool {
    year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
}

fn last_day_of_month(year: i64, month: i32) -> i32 {
    if month == 2 {
        return 28 + is_leap(year) as i32;
    }
    if matches!(month, 4 | 6 | 9 | 11) {
        return 30;
    }
    31
}

fn advance_date(year: &mut i64, month: &mut i32, mday: &mut i32, yday: &mut i32) {
    if *mday != last_day_of_month(*year, *month) {
        *mday += 1;
        *yday += 1;
        return;
    }
    *mday = 1;
    if *month != 12 {
        *month += 1;
        *yday += 1;
        return;
    }
    *month = 1;
    *yday = 0;
    *year += 1;
}

#[test]
fn time64_to_tm_date_range() {
    // 80000 years = (80000 / 400) * 400 years = (80000 / 400) * 146097 days.
    let total_secs: i64 = 80000 / 400 * 146097 * 86400;
    let mut year: i64 = 1970 - 80000;
    let mut month = 1;
    let mut mday = 1;
    let mut yday = 0;

    let mut secs = -total_secs;
    while secs <= total_secs {
        let result = time64_to_tm(secs, 0);
        let days = secs.div_euclid(86400);
        let fail = format!("{year:05}/{month:02}/{mday:02} ({yday:2}) : {days}");
        assert_eq!(i64::from(result.tm_year), year - 1900, "{fail}");
        assert_eq!(result.tm_mon, month - 1, "{fail}");
        assert_eq!(result.tm_mday, mday, "{fail}");
        assert_eq!(result.tm_yday, yday, "{fail}");
        advance_date(&mut year, &mut month, &mut mday, &mut yday);
        secs += 86400;
    }
}

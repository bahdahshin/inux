// SPDX-License-Identifier: GPL-2.0+
//! Port of `lib/tests/util_macros_kunit.c`: every range in the C tables for
//! `find_closest()` and `find_closest_descending()`.
//!
//! Each C `FIND_CLOSEST_RANGE_CHECK(from, to, array, exp)` becomes one
//! `range_check!` line; the arrays keep the C values and their element types.

use rinux_lib::util_macros::{find_closest, find_closest_descending};

/// `FIND_CLOSEST_RANGE_CHECK()`: every `x` in `from..=to` maps to `exp`.
macro_rules! range_check {
    ($find:ident, $from:expr, $to:expr, $arr:expr, $exp:expr) => {
        for x in $from..=$to {
            assert_eq!($find(x, &$arr), $exp, "x = {x}");
        }
    };
}

#[test]
fn test_find_closest() {
    // This will test a few arrays that are found in drivers.
    let ina226_avg_tab: [i32; 8] = [1, 4, 16, 64, 128, 256, 512, 1024];
    let ad7616_oversampling_avail: [u32; 8] = [1, 2, 4, 8, 16, 32, 64, 128];
    let wd_timeout_table: [u32; 8] = [2, 4, 6, 8, 16, 32, 48, 64];
    let array_prog1a: [i32; 5] = [1, 2, 3, 4, 5];
    let array_prog1b: [u32; 5] = [2, 3, 4, 5, 6];
    let array_prog1mix: [i32; 5] = [-2, -1, 0, 1, 2];
    let array_prog2a: [i32; 4] = [1, 3, 5, 7];
    let array_prog2b: [u32; 4] = [2, 4, 6, 8];
    let array_prog3a: [i32; 4] = [1, 4, 7, 10];
    let array_prog3b: [u32; 4] = [2, 5, 8, 11];
    let array_prog4a: [i32; 4] = [1, 5, 9, 13];
    let array_prog4b: [u32; 4] = [2, 6, 10, 14];

    range_check!(find_closest, -3, 2, ina226_avg_tab, 0);
    range_check!(find_closest, 3, 10, ina226_avg_tab, 1);
    range_check!(find_closest, 11, 40, ina226_avg_tab, 2);
    range_check!(find_closest, 41, 96, ina226_avg_tab, 3);
    range_check!(find_closest, 97, 192, ina226_avg_tab, 4);
    range_check!(find_closest, 193, 384, ina226_avg_tab, 5);
    range_check!(find_closest, 385, 768, ina226_avg_tab, 6);
    range_check!(find_closest, 769, 2048, ina226_avg_tab, 7);

    // The array that found the bug that caused this test to exist.
    range_check!(find_closest, -3, 1, ad7616_oversampling_avail, 0);
    range_check!(find_closest, 2, 3, ad7616_oversampling_avail, 1);
    range_check!(find_closest, 4, 6, ad7616_oversampling_avail, 2);
    range_check!(find_closest, 7, 12, ad7616_oversampling_avail, 3);
    range_check!(find_closest, 13, 24, ad7616_oversampling_avail, 4);
    range_check!(find_closest, 25, 48, ad7616_oversampling_avail, 5);
    range_check!(find_closest, 49, 96, ad7616_oversampling_avail, 6);
    range_check!(find_closest, 97, 256, ad7616_oversampling_avail, 7);

    range_check!(find_closest, -3, 3, wd_timeout_table, 0);
    range_check!(find_closest, 4, 5, wd_timeout_table, 1);
    range_check!(find_closest, 6, 7, wd_timeout_table, 2);
    range_check!(find_closest, 8, 12, wd_timeout_table, 3);
    range_check!(find_closest, 13, 24, wd_timeout_table, 4);
    range_check!(find_closest, 25, 40, wd_timeout_table, 5);
    range_check!(find_closest, 41, 56, wd_timeout_table, 6);
    range_check!(find_closest, 57, 128, wd_timeout_table, 7);

    // One could argue that find_closest() should not be used for monotonic
    // arrays (like 1,2,3,4,5), but even so, it should work as long as the
    // array is sorted ascending.
    range_check!(find_closest, -3, 1, array_prog1a, 0);
    range_check!(find_closest, 2, 2, array_prog1a, 1);
    range_check!(find_closest, 3, 3, array_prog1a, 2);
    range_check!(find_closest, 4, 4, array_prog1a, 3);
    range_check!(find_closest, 5, 8, array_prog1a, 4);

    range_check!(find_closest, -3, 2, array_prog1b, 0);
    range_check!(find_closest, 3, 3, array_prog1b, 1);
    range_check!(find_closest, 4, 4, array_prog1b, 2);
    range_check!(find_closest, 5, 5, array_prog1b, 3);
    range_check!(find_closest, 6, 8, array_prog1b, 4);

    range_check!(find_closest, -4, -2, array_prog1mix, 0);
    range_check!(find_closest, -1, -1, array_prog1mix, 1);
    range_check!(find_closest, 0, 0, array_prog1mix, 2);
    range_check!(find_closest, 1, 1, array_prog1mix, 3);
    range_check!(find_closest, 2, 5, array_prog1mix, 4);

    range_check!(find_closest, -3, 2, array_prog2a, 0);
    range_check!(find_closest, 3, 4, array_prog2a, 1);
    range_check!(find_closest, 5, 6, array_prog2a, 2);
    range_check!(find_closest, 7, 10, array_prog2a, 3);

    range_check!(find_closest, -3, 3, array_prog2b, 0);
    range_check!(find_closest, 4, 5, array_prog2b, 1);
    range_check!(find_closest, 6, 7, array_prog2b, 2);
    range_check!(find_closest, 8, 10, array_prog2b, 3);

    range_check!(find_closest, -3, 2, array_prog3a, 0);
    range_check!(find_closest, 3, 5, array_prog3a, 1);
    range_check!(find_closest, 6, 8, array_prog3a, 2);
    range_check!(find_closest, 9, 20, array_prog3a, 3);

    range_check!(find_closest, -3, 3, array_prog3b, 0);
    range_check!(find_closest, 4, 6, array_prog3b, 1);
    range_check!(find_closest, 7, 9, array_prog3b, 2);
    range_check!(find_closest, 10, 20, array_prog3b, 3);

    range_check!(find_closest, -3, 3, array_prog4a, 0);
    range_check!(find_closest, 4, 7, array_prog4a, 1);
    range_check!(find_closest, 8, 11, array_prog4a, 2);
    range_check!(find_closest, 12, 20, array_prog4a, 3);

    range_check!(find_closest, -3, 4, array_prog4b, 0);
    range_check!(find_closest, 5, 8, array_prog4b, 1);
    range_check!(find_closest, 9, 12, array_prog4b, 2);
    range_check!(find_closest, 13, 20, array_prog4b, 3);
}

#[test]
fn test_find_closest_descending() {
    // Same arrays as 'test_find_closest' but reversed.
    let ina226_avg_tab: [i32; 8] = [1024, 512, 256, 128, 64, 16, 4, 1];
    let ad7616_oversampling_avail: [u32; 8] = [128, 64, 32, 16, 8, 4, 2, 1];
    let wd_timeout_table: [u32; 8] = [64, 48, 32, 16, 8, 6, 4, 2];
    let array_prog1a: [i32; 5] = [5, 4, 3, 2, 1];
    let array_prog1b: [u32; 5] = [6, 5, 4, 3, 2];
    let array_prog1mix: [i32; 5] = [2, 1, 0, -1, -2];
    let array_prog2a: [i32; 4] = [7, 5, 3, 1];
    let array_prog2b: [u32; 4] = [8, 6, 4, 2];
    let array_prog3a: [i32; 4] = [10, 7, 4, 1];
    let array_prog3b: [u32; 4] = [11, 8, 5, 2];
    let array_prog4a: [i32; 4] = [13, 9, 5, 1];
    let array_prog4b: [u32; 4] = [14, 10, 6, 2];

    range_check!(find_closest_descending, -3, 2, ina226_avg_tab, 7);
    range_check!(find_closest_descending, 3, 10, ina226_avg_tab, 6);
    range_check!(find_closest_descending, 11, 40, ina226_avg_tab, 5);
    range_check!(find_closest_descending, 41, 96, ina226_avg_tab, 4);
    range_check!(find_closest_descending, 97, 192, ina226_avg_tab, 3);
    range_check!(find_closest_descending, 193, 384, ina226_avg_tab, 2);
    range_check!(find_closest_descending, 385, 768, ina226_avg_tab, 1);
    range_check!(find_closest_descending, 769, 2048, ina226_avg_tab, 0);

    range_check!(find_closest_descending, -3, 1, ad7616_oversampling_avail, 7);
    range_check!(find_closest_descending, 2, 3, ad7616_oversampling_avail, 6);
    range_check!(find_closest_descending, 4, 6, ad7616_oversampling_avail, 5);
    range_check!(find_closest_descending, 7, 12, ad7616_oversampling_avail, 4);
    range_check!(
        find_closest_descending,
        13,
        24,
        ad7616_oversampling_avail,
        3
    );
    range_check!(
        find_closest_descending,
        25,
        48,
        ad7616_oversampling_avail,
        2
    );
    range_check!(
        find_closest_descending,
        49,
        96,
        ad7616_oversampling_avail,
        1
    );
    range_check!(
        find_closest_descending,
        97,
        256,
        ad7616_oversampling_avail,
        0
    );

    range_check!(find_closest_descending, -3, 3, wd_timeout_table, 7);
    range_check!(find_closest_descending, 4, 5, wd_timeout_table, 6);
    range_check!(find_closest_descending, 6, 7, wd_timeout_table, 5);
    range_check!(find_closest_descending, 8, 12, wd_timeout_table, 4);
    range_check!(find_closest_descending, 13, 24, wd_timeout_table, 3);
    range_check!(find_closest_descending, 25, 40, wd_timeout_table, 2);
    range_check!(find_closest_descending, 41, 56, wd_timeout_table, 1);
    range_check!(find_closest_descending, 57, 128, wd_timeout_table, 0);

    // One could argue that find_closest_descending() should not be used for
    // monotonic arrays (like 5,4,3,2,1), but even so, it should still work
    // as long as the array is sorted descending.
    range_check!(find_closest_descending, -3, 1, array_prog1a, 4);
    range_check!(find_closest_descending, 2, 2, array_prog1a, 3);
    range_check!(find_closest_descending, 3, 3, array_prog1a, 2);
    range_check!(find_closest_descending, 4, 4, array_prog1a, 1);
    range_check!(find_closest_descending, 5, 8, array_prog1a, 0);

    range_check!(find_closest_descending, -3, 2, array_prog1b, 4);
    range_check!(find_closest_descending, 3, 3, array_prog1b, 3);
    range_check!(find_closest_descending, 4, 4, array_prog1b, 2);
    range_check!(find_closest_descending, 5, 5, array_prog1b, 1);
    range_check!(find_closest_descending, 6, 8, array_prog1b, 0);

    range_check!(find_closest_descending, -4, -2, array_prog1mix, 4);
    range_check!(find_closest_descending, -1, -1, array_prog1mix, 3);
    range_check!(find_closest_descending, 0, 0, array_prog1mix, 2);
    range_check!(find_closest_descending, 1, 1, array_prog1mix, 1);
    range_check!(find_closest_descending, 2, 5, array_prog1mix, 0);

    range_check!(find_closest_descending, -3, 2, array_prog2a, 3);
    range_check!(find_closest_descending, 3, 4, array_prog2a, 2);
    range_check!(find_closest_descending, 5, 6, array_prog2a, 1);
    range_check!(find_closest_descending, 7, 10, array_prog2a, 0);

    range_check!(find_closest_descending, -3, 3, array_prog2b, 3);
    range_check!(find_closest_descending, 4, 5, array_prog2b, 2);
    range_check!(find_closest_descending, 6, 7, array_prog2b, 1);
    range_check!(find_closest_descending, 8, 10, array_prog2b, 0);

    range_check!(find_closest_descending, -3, 2, array_prog3a, 3);
    range_check!(find_closest_descending, 3, 5, array_prog3a, 2);
    range_check!(find_closest_descending, 6, 8, array_prog3a, 1);
    range_check!(find_closest_descending, 9, 20, array_prog3a, 0);

    range_check!(find_closest_descending, -3, 3, array_prog3b, 3);
    range_check!(find_closest_descending, 4, 6, array_prog3b, 2);
    range_check!(find_closest_descending, 7, 9, array_prog3b, 1);
    range_check!(find_closest_descending, 10, 20, array_prog3b, 0);

    range_check!(find_closest_descending, -3, 3, array_prog4a, 3);
    range_check!(find_closest_descending, 4, 7, array_prog4a, 2);
    range_check!(find_closest_descending, 8, 11, array_prog4a, 1);
    range_check!(find_closest_descending, 12, 20, array_prog4a, 0);

    range_check!(find_closest_descending, -3, 4, array_prog4b, 3);
    range_check!(find_closest_descending, 5, 8, array_prog4b, 2);
    range_check!(find_closest_descending, 9, 12, array_prog4b, 1);
    range_check!(find_closest_descending, 13, 20, array_prog4b, 0);
}

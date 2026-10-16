// SPDX-License-Identifier: GPL-2.0
// Copyright (C) 2020, ROHM Semiconductors.
//! Port of `lib/tests/test_linear_ranges.c`, the KUnit test for the
//! linear_ranges helper. The same two ranges and the same few cases are used.

use rinux_lib::errno::EINVAL;
use rinux_lib::linear_ranges::{
    LinearRange, linear_range_get_selector_high, linear_range_get_selector_low_array,
    linear_range_get_value_array, linear_range_values_in_range_array,
};

// Test Range 1: selectors 2..=6 give values 10..=50 in steps of 10.
const RANGE1_MIN: u32 = 10;
const RANGE1_MIN_SEL: u32 = 2;
const RANGE1_STEP: u32 = 10;
const RANGE1_SELS: [u32; 5] = [2, 3, 4, 5, 6];
const RANGE1_VALS: [u32; 5] = [10, 20, 30, 40, 50];

// Test Range 2: selectors 7..=10 give values 100..=250 in steps of 50.
const RANGE2_MIN: u32 = 100;
const RANGE2_MIN_SEL: u32 = 7;
const RANGE2_STEP: u32 = 50;
const RANGE2_SELS: [u32; 4] = [7, 8, 9, 10];
const RANGE2_VALS: [u32; 4] = [100, 150, 200, 250];

const RANGE_NUM_VALS: u32 = (RANGE1_VALS.len() + RANGE2_VALS.len()) as u32;

fn test_ranges() -> [LinearRange; 2] {
    let range1_max_sel = RANGE1_MIN_SEL + RANGE1_VALS.len() as u32 - 1;
    let range2_max_sel = RANGE2_MIN_SEL + RANGE2_VALS.len() as u32 - 1;
    [
        LinearRange::new(RANGE1_MIN, RANGE1_MIN_SEL, range1_max_sel, RANGE1_STEP),
        LinearRange::new(RANGE2_MIN, RANGE2_MIN_SEL, range2_max_sel, RANGE2_STEP),
    ]
}

#[test]
fn range_test_get_value() {
    let testr = test_ranges();
    for (sel, val) in RANGE1_SELS.iter().zip(RANGE1_VALS) {
        assert_eq!(linear_range_get_value_array(&testr, *sel), Ok(val));
    }
    for (sel, val) in RANGE2_SELS.iter().zip(RANGE2_VALS) {
        assert_eq!(linear_range_get_value_array(&testr, *sel), Ok(val));
    }
    // One past the last selector is outside every range.
    let last = RANGE2_SELS[RANGE2_SELS.len() - 1];
    assert_eq!(linear_range_get_value_array(&testr, last + 1), Err(EINVAL));
}

#[test]
fn range_test_get_selector_high() {
    let testr = test_ranges();
    let range1 = testr[0];
    for (val, sel) in RANGE1_VALS.iter().zip(RANGE1_SELS) {
        let mut s = 0;
        let mut found = false;
        assert_eq!(
            linear_range_get_selector_high(&range1, *val, &mut s, &mut found),
            Ok(())
        );
        assert_eq!(s, sel);
        assert!(found);
    }

    let mut s = 0;
    let mut found = false;
    let max_val = RANGE1_VALS[RANGE1_VALS.len() - 1];
    assert_eq!(
        linear_range_get_selector_high(&range1, max_val + 1, &mut s, &mut found),
        Err(EINVAL)
    );

    // Below the range: Ok, selector is the lowest one, but not found.
    assert_eq!(
        linear_range_get_selector_high(&range1, RANGE1_MIN - 1, &mut s, &mut found),
        Ok(())
    );
    assert!(!found);
    assert_eq!(s, RANGE1_SELS[0]);
}

#[test]
fn range_test_get_value_amount() {
    let testr = test_ranges();
    assert_eq!(linear_range_values_in_range_array(&testr), RANGE_NUM_VALS);
}

#[test]
fn range_test_get_selector_low() {
    let testr = test_ranges();
    for (val, sel) in RANGE1_VALS.iter().zip(RANGE1_SELS) {
        let mut s = 0;
        let mut found = false;
        assert_eq!(
            linear_range_get_selector_low_array(&testr, *val, &mut s, &mut found),
            Ok(())
        );
        assert_eq!(s, sel);
        assert!(found);
    }
    for (val, sel) in RANGE2_VALS.iter().zip(RANGE2_SELS) {
        let mut s = 0;
        let mut found = false;
        assert_eq!(
            linear_range_get_selector_low_array(&testr, *val, &mut s, &mut found),
            Ok(())
        );
        assert_eq!(s, sel);
        assert!(found);
    }

    // Above the top of the range: Ok, the highest selector, but not found.
    let mut s = 0;
    let mut found = false;
    let above = RANGE2_VALS[RANGE2_VALS.len() - 1] + 1;
    assert_eq!(
        linear_range_get_selector_low_array(&testr, above, &mut s, &mut found),
        Ok(())
    );
    assert_eq!(s, RANGE2_SELS[RANGE2_SELS.len() - 1]);
    assert!(!found);
}

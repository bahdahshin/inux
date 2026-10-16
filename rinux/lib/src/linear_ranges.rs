// SPDX-License-Identifier: GPL-2.0
//! Linear ranges: map between selector register values and physical values.
//!
//! Ported from `lib/linear_ranges.c` and `include/linux/linear_range.h`,
//! Copyright (C) 2020 ROHM Semiconductors. A range maps selectors
//! `min_sel..=max_sel` linearly onto values `min, min + step, ...`.
//!
//! The C functions return an `int` and write results through pointers. The
//! functions here keep that shape: the `Result` carries the return code, and
//! `&mut` outputs carry `*selector` and `*found`. Arrays of ranges are slices.

use crate::errno::{EINVAL, Errno};

/// `struct linear_range`: one linear segment of a selector table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LinearRange {
    /// The value at selector `min_sel`.
    pub min: u32,
    /// The lowest selector in the range.
    pub min_sel: u32,
    /// The highest selector in the range.
    pub max_sel: u32,
    /// The value increment per selector step.
    pub step: u32,
}

impl LinearRange {
    /// `LINEAR_RANGE()`.
    pub const fn new(min: u32, min_sel: u32, max_sel: u32, step: u32) -> Self {
        Self {
            min,
            min_sel,
            max_sel,
            step,
        }
    }
}

/// `linear_range_values_in_range()`: the number of selectors in `r`.
pub fn linear_range_values_in_range(r: &LinearRange) -> u32 {
    r.max_sel.wrapping_sub(r.min_sel).wrapping_add(1)
}

/// `linear_range_values_in_range_array()`: the total selectors in `ranges`.
///
/// Returns 0 as soon as one range has no selectors, as the C code does.
pub fn linear_range_values_in_range_array(ranges: &[LinearRange]) -> u32 {
    let mut values_in_range: u32 = 0;
    for r in ranges {
        let values = linear_range_values_in_range(r);
        if values == 0 {
            return values;
        }
        values_in_range = values_in_range.wrapping_add(values);
    }
    values_in_range
}

/// `linear_range_get_max_value()`: the value at the highest selector.
pub fn linear_range_get_max_value(r: &LinearRange) -> u32 {
    r.min
        .wrapping_add(r.max_sel.wrapping_sub(r.min_sel).wrapping_mul(r.step))
}

/// `linear_range_get_value()`: the value for `selector`, if `r` covers it.
pub fn linear_range_get_value(r: &LinearRange, selector: u32) -> Result<u32, Errno> {
    if r.min_sel > selector || r.max_sel < selector {
        return Err(EINVAL);
    }
    Ok(r.min
        .wrapping_add(selector.wrapping_sub(r.min_sel).wrapping_mul(r.step)))
}

/// `linear_range_get_value_array()`: the value for `selector` in the first
/// range that covers it.
pub fn linear_range_get_value_array(ranges: &[LinearRange], selector: u32) -> Result<u32, Errno> {
    for r in ranges {
        if r.min_sel <= selector && r.max_sel >= selector {
            return linear_range_get_value(r, selector);
        }
    }
    Err(EINVAL)
}

/// `linear_range_get_selector_low()`: the largest selector whose value is
/// at most `val`. `found` is set when `val` lies inside the range.
pub fn linear_range_get_selector_low(
    r: &LinearRange,
    val: u32,
    selector: &mut u32,
    found: &mut bool,
) -> Result<(), Errno> {
    *found = false;

    if r.min > val {
        return Err(EINVAL);
    }

    if linear_range_get_max_value(r) < val {
        *selector = r.max_sel;
        return Ok(());
    }

    *found = true;

    *selector = if r.step == 0 {
        r.min_sel
    } else {
        (val - r.min) / r.step + r.min_sel
    };
    Ok(())
}

/// `linear_range_get_selector_low_array()`: [`linear_range_get_selector_low`]
/// over several ranges, stopping at the first range that contains `val`.
pub fn linear_range_get_selector_low_array(
    ranges: &[LinearRange],
    val: u32,
    selector: &mut u32,
    found: &mut bool,
) -> Result<(), Errno> {
    let mut ret = Err(EINVAL);
    for r in ranges {
        let tmpret = linear_range_get_selector_low(r, val, selector, found);
        if tmpret.is_ok() {
            ret = Ok(());
        }
        if *found {
            break;
        }
    }
    ret
}

/// `linear_range_get_selector_high()`: the smallest selector whose value is
/// at least `val`. `found` is set when `val` lies inside the range.
pub fn linear_range_get_selector_high(
    r: &LinearRange,
    val: u32,
    selector: &mut u32,
    found: &mut bool,
) -> Result<(), Errno> {
    *found = false;

    if linear_range_get_max_value(r) < val {
        return Err(EINVAL);
    }

    if r.min > val {
        *selector = r.min_sel;
        return Ok(());
    }

    *found = true;

    *selector = if r.step == 0 {
        r.max_sel
    } else {
        (val - r.min).div_ceil(r.step) + r.min_sel
    };
    Ok(())
}

/// `linear_range_get_selector_high_array()`: [`linear_range_get_selector_high`]
/// over several ranges, returning the first range that succeeds.
pub fn linear_range_get_selector_high_array(
    ranges: &[LinearRange],
    val: u32,
    selector: &mut u32,
    found: &mut bool,
) -> Result<(), Errno> {
    for r in ranges {
        if linear_range_get_selector_high(r, val, selector, found).is_ok() {
            return Ok(());
        }
    }
    Err(EINVAL)
}

/// `linear_range_get_selector_within()`: the selector for `val`, clamped to
/// the range's selectors. Values below the range give `min_sel`, values above
/// give `max_sel`, and in between the selector rounds down.
pub fn linear_range_get_selector_within(r: &LinearRange, val: u32, selector: &mut u32) {
    if r.min > val {
        *selector = r.min_sel;
        return;
    }
    if linear_range_get_max_value(r) < val {
        *selector = r.max_sel;
        return;
    }
    *selector = if r.step == 0 {
        r.min_sel
    } else {
        (val - r.min) / r.step + r.min_sel
    };
}

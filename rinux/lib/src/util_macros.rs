// SPDX-License-Identifier: GPL-2.0+
//! Array search helpers from `include/linux/util_macros.h`.
//!
//! [`find_closest`] and [`find_closest_descending`] are the `find_closest()`
//! and `find_closest_descending()` macros. C's macros take the array and
//! its size; here the slice carries both. The C macros compute the midpoint
//! of each adjacent pair in `long`, so the elements are widened to `i64`
//! before the arithmetic, and `x` is passed as `i64`.
//!
//! Both return the index of the element closest to `x`, using the midpoint
//! between neighbours: `x` at or below a midpoint belongs to the lower
//! element, and a tie goes to the lower index. An empty slice has no closest
//! element, where C reads past the array; here it panics.

/// `find_closest()`: the index of the element of `a` closest to `x`, for `a`
/// sorted in ascending order.
pub fn find_closest<T: Copy + Into<i64>>(x: i64, a: &[T]) -> usize {
    assert!(!a.is_empty(), "find_closest: empty array");
    let last = a.len() - 1;
    let mut i = 0;
    while i < last {
        let (lo, hi): (i64, i64) = (a[i].into(), a[i + 1].into());
        if x <= (lo + hi) / 2 {
            let left = x - lo;
            let right = hi - x;
            if right < left {
                i += 1;
            }
            break;
        }
        i += 1;
    }
    i
}

/// `find_closest_descending()`: the index of the element of `a` closest to
/// `x`, for `a` sorted in descending order. The scan runs from the back, so
/// the comparison of the two distances also works for unsigned elements.
pub fn find_closest_descending<T: Copy + Into<i64>>(x: i64, a: &[T]) -> usize {
    assert!(!a.is_empty(), "find_closest_descending: empty array");
    let mut i = a.len() - 1;
    while i >= 1 {
        let (cur, prev): (i64, i64) = (a[i].into(), a[i - 1].into());
        if x <= (cur + prev) / 2 {
            let left = x - cur;
            let right = prev - x;
            if right < left {
                i -= 1;
            }
            break;
        }
        i -= 1;
    }
    i
}

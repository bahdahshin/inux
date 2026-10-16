// SPDX-License-Identifier: GPL-2.0-only
//! Binary search.
//!
//! Ported from `lib/bsearch.c` and `include/linux/bsearch.h`.
//!
//! Copyright (C) 2008-2009 Ksplice, Inc.
//! Author: Tim Abbott <tabbott@ksplice.com>

use core::cmp::Ordering;

/// `bsearch()`: finds an element of the sorted slice `base` matching `key`.
///
/// `cmp(key, elem)` orders the key against an element, so the key need not
/// have the element type: it can be a name compared with a struct's name
/// field. Returns the index of a match, if there is one.
pub fn bsearch<K, T, C>(key: &K, base: &[T], mut cmp: C) -> Option<usize>
where
    K: ?Sized,
    C: FnMut(&K, &T) -> Ordering,
{
    let mut lo = 0;
    let mut num = base.len();

    while num > 0 {
        let pivot = lo + (num >> 1);
        match cmp(key, &base[pivot]) {
            Ordering::Equal => return Some(pivot),
            Ordering::Greater => {
                lo = pivot + 1;
                num -= 1;
            }
            Ordering::Less => {}
        }
        num >>= 1;
    }
    None
}

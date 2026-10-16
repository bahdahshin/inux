// SPDX-License-Identifier: GPL-2.0
//! In-place heapsort.
//!
//! Ported from `lib/sort.c`: a fast, small, non-recursive O(n log n) sort.
//! It makes n*log2(n) + 0.37*n + o(n) comparisons on average and
//! 1.5*n*log2(n) + O(n) in the (very contrived) worst case. Quicksort is a
//! little faster on random input but has an O(n^2) worst case and needs
//! stack, which the kernel can't afford.
//!
//! The sort is not stable. `sort_r()`'s `priv` argument is not needed in
//! Rust: closures capture whatever context they need. The `_nonatomic`
//! variants, which call `cond_resched()`, will come with the scheduler.

use core::cmp::Ordering;

/// `parent()`: index of the heap parent of `i`, which must be non-zero.
#[inline]
fn parent(i: usize) -> usize {
    (i - 1) / 2
}

/// `sort()`: sorts `base` in place with comparison `cmp`.
pub fn sort<T, C>(base: &mut [T], cmp: C)
where
    C: FnMut(&T, &T) -> Ordering,
{
    sort_with_swap(base, cmp, |s, a, b| s.swap(a, b));
}

/// `sort()` with a custom swap function.
///
/// `swap(base, a, b)` must exchange elements `a` and `b`, and may update
/// other state, for example an index into the array. It can be called with
/// `a == b`.
pub fn sort_with_swap<T, C, S>(base: &mut [T], mut cmp: C, mut swap: S)
where
    C: FnMut(&T, &T) -> Ordering,
    S: FnMut(&mut [T], usize, usize),
{
    let mut n = base.len();
    let mut a = n / 2;
    let mut shift = 0;

    if a == 0 {
        // Fewer than two elements.
        return;
    }

    // Loop invariants:
    // 1. elements [a, n) satisfy the heap property (compare greater than
    //    all of their children),
    // 2. elements [n, len) are sorted, and
    // 3. a <= b <= c <= d <= n (whenever they are valid).
    loop {
        if a != 0 {
            // Building the heap: sift down a.
            a -= 1 << shift;
        } else if n > 3 {
            // Sorting: extract the two largest elements.
            n -= 1;
            swap(base, 0, n);
            shift = (cmp(&base[1], &base[2]) != Ordering::Greater) as u32;
            a = 1 << shift;
            n -= 1;
            swap(base, a, n);
        } else {
            break;
        }

        // Sift the element at a down into the heap. This is the bottom-up
        // variant: find the sift-down path all the way to the leaves (one
        // compare per level), then backtrack to where a belongs. Elements
        // tend to sift down close to the leaves, so this does fewer compares
        // than two per level on the way down.
        let mut b = a;
        let (mut c, mut d);
        loop {
            c = 2 * b + 1;
            d = c + 1;
            if d >= n {
                break;
            }
            b = if cmp(&base[c], &base[d]) == Ordering::Greater {
                c
            } else {
                d
            };
        }
        if d == n {
            // The last leaf has no sibling.
            b = c;
        }

        // Backtrack from b to the correct location for a.
        while b != a && cmp(&base[a], &base[b]) != Ordering::Less {
            b = parent(b);
        }
        c = b;
        // Shift it into place.
        while b != a {
            b = parent(b);
            swap(base, b, c);
        }
    }

    n -= 1;
    swap(base, 0, n);
    if n == 2 && cmp(&base[0], &base[1]) == Ordering::Greater {
        swap(base, 0, 1);
    }
}

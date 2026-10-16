// SPDX-License-Identifier: GPL-2.0
//! Concatenate two pointer arrays into a caller's buffer.
//!
//! Ported from `lib/memcat_p.c` and the `memcat_p()` macro in
//! `include/linux/string.h`. The C function takes two NULL-terminated
//! arrays and allocates a new NULL-terminated array holding both. Here an
//! array is a slice, so its end replaces the NULL, and the result goes into
//! a buffer the caller provides, so nothing is allocated.

/// `__memcat_p()`: write `a` followed by `b` into `out`.
///
/// Returns the number of elements written, which is `a.len() + b.len()`, or
/// `None` when `out` is too small (C returns NULL when `kmalloc_array()`
/// fails). Nothing is written in that case.
pub fn memcat_p<T: Copy>(a: &[T], b: &[T], out: &mut [T]) -> Option<usize> {
    let total = a.len().checked_add(b.len())?;
    if out.len() < total {
        return None;
    }
    out[..a.len()].copy_from_slice(a);
    out[a.len()..total].copy_from_slice(b);
    Some(total)
}

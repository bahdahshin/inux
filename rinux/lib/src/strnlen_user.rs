// SPDX-License-Identifier: GPL-2.0
//! The length of a NUL-terminated string in user memory, bounded by a count.
//!
//! Ported from `lib/strnlen_user.c`. The result counts the terminating NUL.
//! It is 0 on a fault (or a non-positive count), `count + 1` when no NUL
//! arrives within `count` bytes, and otherwise the length plus one. The
//! word-at-a-time search in C changes only speed, so this is the byte loop.
//! User memory is reached through `read`, which returns `None` for a fault.

/// `strnlen_user()`: see the module docs for the result convention.
pub fn strnlen_user(count: i64, mut read: impl FnMut(usize) -> Option<u8>) -> i64 {
    if count <= 0 {
        return 0;
    }
    let count = count as usize;
    for res in 0..count {
        match read(res) {
            None => return 0,
            Some(0) => return res as i64 + 1,
            Some(_) => {}
        }
    }
    // No NUL within `count` bytes: the caller's "too long" marker.
    count as i64 + 1
}

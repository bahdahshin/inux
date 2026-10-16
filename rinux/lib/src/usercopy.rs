// SPDX-License-Identifier: GPL-2.0
//! Checks on user memory: whether a user range is all zero.
//!
//! Ported from `lib/usercopy.c`, `check_zeroed_user()`. C reads user memory a
//! word at a time from the aligned address below the range and masks the
//! bytes outside it. The result is the same as checking each byte of the
//! range, which is what is written here, with the same early exit: the first
//! non-zero byte ends the check, so bytes after it are never read. Reads go
//! through `read`, which returns `None` for a fault.

use crate::errno::EFAULT;

/// `check_zeroed_user()`: 1 when `size` bytes of user memory from offset 0 are
/// all zero, 0 when one is not, and `-EFAULT` when a read faults before a
/// non-zero byte is found.
pub fn check_zeroed_user(size: usize, mut read: impl FnMut(usize) -> Option<u8>) -> i32 {
    for i in 0..size {
        match read(i) {
            None => return -(EFAULT.get()),
            Some(0) => {}
            Some(_) => return 0,
        }
    }
    1
}

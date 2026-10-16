// SPDX-License-Identifier: GPL-2.0-or-later
//! Host tests for `check_zeroed_user`, over a fake user memory. Linux has a
//! KUnit file for this (`usercopy_kunit.c`), which is not ported yet.

use rinux_lib::errno::EFAULT;
use rinux_lib::usercopy::check_zeroed_user;

fn user(mem: &'static [u8]) -> impl FnMut(usize) -> Option<u8> {
    move |a| mem.get(a).copied()
}

#[test]
fn all_zero_range_is_one() {
    assert_eq!(check_zeroed_user(5, user(&[0, 0, 0, 0, 0, 9])), 1);
    assert_eq!(check_zeroed_user(0, user(&[])), 1);
}

#[test]
fn a_nonzero_byte_is_zero() {
    assert_eq!(check_zeroed_user(6, user(&[0, 0, 0, 0, 0, 9])), 0);
    assert_eq!(check_zeroed_user(3, user(&[7, 0, 0])), 0);
}

#[test]
fn a_fault_before_a_nonzero_byte_is_efault() {
    assert_eq!(check_zeroed_user(4, user(&[0, 0])), -EFAULT.get());
}

#[test]
fn stops_reading_at_the_first_nonzero_byte() {
    let mut reads = 0;
    let r = check_zeroed_user(10, |i| {
        reads += 1;
        Some(if i == 2 { 1 } else { 0 })
    });
    assert_eq!(r, 0);
    assert_eq!(reads, 3);
}

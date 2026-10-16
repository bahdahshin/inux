// SPDX-License-Identifier: GPL-2.0-or-later
//! Host tests for `strncpy_from_user`, over a fake user memory. Linux has a
//! KUnit file for this (`usercopy_kunit.c`), which is not ported yet.

use rinux_lib::errno::EFAULT;
use rinux_lib::strncpy_from_user::strncpy_from_user;

fn user(mem: &'static [u8]) -> impl FnMut(usize) -> Option<u8> {
    move |a| mem.get(a).copied()
}

#[test]
fn copies_up_to_the_nul_and_reports_its_length() {
    let mut dst = [0xffu8; 16];
    let n = strncpy_from_user(&mut dst, 16, user(b"hello\0world")).unwrap();
    assert_eq!(n, 5);
    assert_eq!(&dst[..6], b"hello\0");
}

#[test]
fn stops_at_count_without_a_nul() {
    let mut dst = [0u8; 8];
    let n = strncpy_from_user(&mut dst, 3, user(b"abcdef")).unwrap();
    assert_eq!(n, 3);
    assert_eq!(&dst[..3], b"abc");
}

#[test]
fn an_empty_string_and_a_zero_count() {
    let mut dst = [0u8; 4];
    assert_eq!(strncpy_from_user(&mut dst, 4, user(b"\0")).unwrap(), 0);
    assert_eq!(strncpy_from_user(&mut dst, 0, user(b"abc")).unwrap(), 0);
}

#[test]
fn a_fault_before_count_is_efault() {
    let mut dst = [0u8; 8];
    // The string runs off the end of user memory before any NUL.
    assert_eq!(strncpy_from_user(&mut dst, 8, user(b"abc")), Err(EFAULT));
}

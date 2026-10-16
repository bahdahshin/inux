// SPDX-License-Identifier: GPL-2.0-or-later
//! Host tests for `strnlen_user`, over a fake user memory. Linux has a KUnit
//! file for this (`usercopy_kunit.c`), which is not ported yet.

use rinux_lib::strnlen_user::strnlen_user;

fn user(mem: &'static [u8]) -> impl FnMut(usize) -> Option<u8> {
    move |a| mem.get(a).copied()
}

#[test]
fn length_includes_the_nul() {
    assert_eq!(strnlen_user(16, user(b"hello\0world")), 6);
    assert_eq!(strnlen_user(16, user(b"\0")), 1);
}

#[test]
fn too_long_returns_count_plus_one() {
    assert_eq!(strnlen_user(3, user(b"abcdef\0")), 4);
}

#[test]
fn a_fault_or_bad_count_returns_zero() {
    assert_eq!(strnlen_user(8, user(b"abc")), 0);
    assert_eq!(strnlen_user(0, user(b"abc\0")), 0);
    assert_eq!(strnlen_user(-1, user(b"abc\0")), 0);
}

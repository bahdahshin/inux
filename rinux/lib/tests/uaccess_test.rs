// SPDX-License-Identifier: GPL-2.0
//! Host tests for the user-memory wrappers in `lib/src/uaccess.rs`: the
//! string helpers run over `FlatUser`, and must give the same answers as
//! their closure-based forms.

use rinux_lib::errno::EFAULT;
use rinux_lib::uaccess::{
    FlatUser, check_zeroed_user_mem, strncpy_from_user_mem, strnlen_user_mem,
};

const BASE: usize = 0x2000;

#[test]
fn strncpy_copies_to_the_nul_and_within_count() {
    let mut mem = *b"abc\0def";
    let user = FlatUser::new(&mut mem, BASE);
    let mut dst = [0u8; 8];
    assert_eq!(strncpy_from_user_mem(&user, &mut dst, 8, BASE), Ok(3));
    assert_eq!(&dst[..3], b"abc");

    let mut dst = [0u8; 8];
    assert_eq!(strncpy_from_user_mem(&user, &mut dst, 2, BASE), Ok(2));
    assert_eq!(&dst[..2], b"ab");
}

#[test]
fn strncpy_faults_when_the_string_runs_off_the_mapping() {
    let mut mem = *b"xy";
    let user = FlatUser::new(&mut mem, BASE);
    let mut dst = [0u8; 8];
    assert_eq!(strncpy_from_user_mem(&user, &mut dst, 8, BASE), Err(EFAULT));
}

#[test]
fn strnlen_counts_with_the_nul_and_faults_as_zero() {
    let mut mem = *b"hello\0";
    let user = FlatUser::new(&mut mem, BASE);
    assert_eq!(strnlen_user_mem(&user, 100, BASE), 6);
    assert_eq!(strnlen_user_mem(&user, 100, 0x10), 0);
}

#[test]
fn check_zeroed_passes_on_zeros_and_stops_at_the_first_other_byte() {
    let mut zeros = [0u8; 16];
    let user = FlatUser::new(&mut zeros, BASE);
    assert_eq!(check_zeroed_user_mem(&user, 16, BASE), 1);

    let mut mixed = [0u8, 0, 1, 0];
    let user = FlatUser::new(&mut mixed, BASE);
    assert_eq!(check_zeroed_user_mem(&user, 4, BASE), 0);
}

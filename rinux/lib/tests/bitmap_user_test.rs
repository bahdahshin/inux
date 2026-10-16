// SPDX-License-Identifier: GPL-2.0
//! Host tests for `bitmap_parse_user()` and `bitmap_parselist_user()`
//! (`lib/bitmap-str.c`) over `FlatUser`. They need the `alloc` feature for the
//! NUL-terminated copy, so this file builds only with it.
#![cfg(feature = "alloc")]

use rinux_lib::alloc_helpers::{bitmap_parse_user, bitmap_parselist_user, memdup_user_nul};
use rinux_lib::errno::{EFAULT, EINVAL};
use rinux_lib::uaccess::FlatUser;

const BASE: usize = 0x3000;

fn with_user<T>(text: &[u8], f: impl FnOnce(&FlatUser<'_>) -> T) -> T {
    let mut mem = text.to_vec();
    let user = FlatUser::new(&mut mem, BASE);
    f(&user)
}

#[test]
fn memdup_copies_and_nul_terminates() {
    with_user(b"abc", |u| {
        let buf = memdup_user_nul(u, BASE, 3).unwrap();
        assert_eq!(buf, b"abc\0");
    });
}

#[test]
fn memdup_faults_when_the_copy_runs_off_the_mapping() {
    with_user(b"ab", |u| {
        assert_eq!(memdup_user_nul(u, BASE, 3), Err(EFAULT))
    });
}

#[test]
fn hex_bitmap_parses_from_user_memory() {
    let mut mask = [0u64; 1];
    with_user(b"ff\n", |u| {
        assert_eq!(bitmap_parse_user(u, BASE, 3, &mut mask, 8), Ok(()));
    });
    assert_eq!(mask[0], 0xff);
}

#[test]
fn list_bitmap_parses_from_user_memory() {
    let mut mask = [0u64; 1];
    with_user(b"0-2,5", |u| {
        assert_eq!(bitmap_parselist_user(u, BASE, 5, &mut mask, 8), Ok(()));
    });
    assert_eq!(mask[0], 0b10_0111);
}

#[test]
fn a_bad_string_is_einval() {
    let mut mask = [0u64; 1];
    with_user(b"zz", |u| {
        assert_eq!(bitmap_parse_user(u, BASE, 2, &mut mask, 8), Err(EINVAL));
    });
}

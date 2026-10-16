// SPDX-License-Identifier: GPL-2.0-or-later
//! Host tests for `memcat_p`. Linux has no KUnit file for it.

use rinux_lib::memcat_p::memcat_p;

#[test]
fn concatenates_both_arrays_in_order() {
    let a = [1u32, 2, 3];
    let b = [4u32, 5];
    let mut out = [0u32; 8];
    assert_eq!(memcat_p(&a, &b, &mut out), Some(5));
    assert_eq!(&out[..5], &[1, 2, 3, 4, 5]);
}

#[test]
fn an_empty_side_gives_the_other_side() {
    let mut out = [0u8; 4];
    assert_eq!(memcat_p(&[], &[9u8, 8], &mut out), Some(2));
    assert_eq!(&out[..2], &[9, 8]);
    assert_eq!(memcat_p(&[7u8], &[], &mut out), Some(1));
    assert_eq!(out[0], 7);
}

#[test]
fn a_small_buffer_is_refused_without_writing() {
    let mut out = [0xaau8; 2];
    assert_eq!(memcat_p(&[1u8, 2], &[3], &mut out), None);
    assert_eq!(out, [0xaa, 0xaa]);
}

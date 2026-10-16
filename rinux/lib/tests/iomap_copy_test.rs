// SPDX-License-Identifier: GPL-2.0-or-later
//! Host tests for the MMIO block copies, over a simulated register file.
//! Linux has no KUnit file for `iomap_copy.c`.

use rinux_lib::iomap_copy::{ioread32_copy, iowrite32_copy, iowrite64_copy};

#[test]
fn writes_go_to_successive_addresses() {
    let mut device = [0u32; 4];
    iowrite32_copy(&[1, 2, 3, 4], |i, v| device[i] = v);
    assert_eq!(device, [1, 2, 3, 4]);
}

#[test]
fn reads_come_from_successive_addresses() {
    let device = [10u32, 20, 30];
    let mut out = [0u32; 3];
    ioread32_copy(&mut out, |i| device[i]);
    assert_eq!(out, [10, 20, 30]);
}

#[test]
fn a_write_then_read_round_trips_64_bit_words() {
    let src = [0x0123_4567_89ab_cdefu64, u64::MAX, 0];
    let mut device = [0u64; 3];
    iowrite64_copy(&src, |i, v| device[i] = v);
    assert_eq!(device, src);
}

#[test]
fn an_empty_copy_touches_nothing() {
    let mut calls = 0;
    iowrite32_copy(&[], |_, _| calls += 1);
    assert_eq!(calls, 0);
}

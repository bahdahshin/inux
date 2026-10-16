// SPDX-License-Identifier: GPL-2.0-or-later
//! Host tests for the I/O memory byte operations, over a simulated device
//! memory. Linux has no KUnit file for `iomem_copy.c`.

use rinux_lib::iomem_copy::{memcpy_fromio, memcpy_toio, memset_io};

#[test]
fn memset_fills_the_whole_range() {
    let mut dev = [0u8; 13];
    memset_io(13, 0xa5, |off, b| dev[off] = b);
    assert!(dev.iter().all(|&b| b == 0xa5));
}

#[test]
fn toio_then_fromio_round_trips() {
    let mut dev = [0u8; 9];
    let src: Vec<u8> = (1..=9).collect();
    memcpy_toio(&src, |off, b| dev[off] = b);
    let mut back = [0u8; 9];
    memcpy_fromio(&mut back, |off| dev[off]);
    assert_eq!(back.to_vec(), src);
}

#[test]
fn empty_ranges_touch_nothing() {
    let mut calls = 0;
    memset_io(0, 1, |_, _| calls += 1);
    memcpy_toio(&[], |_, _| calls += 1);
    memcpy_fromio(&mut [], |_| {
        calls += 1;
        0
    });
    assert_eq!(calls, 0);
}

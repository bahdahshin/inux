// SPDX-License-Identifier: GPL-2.0-or-later
//! Host tests for `check_signature`. Linux has no KUnit file for it.

use rinux_lib::check_signature::check_signature;

#[test]
fn matching_signature_is_found() {
    let memory = b"xx_PCI$_yy";
    assert!(check_signature(|i| memory[3 + i], b"PCI$"));
}

#[test]
fn a_mismatch_at_any_byte_fails() {
    let memory = b"PCI!";
    assert!(!check_signature(|i| memory[i], b"PCI$"));
    assert!(!check_signature(|i| memory[i], b"XCI!"));
}

#[test]
fn empty_signature_always_matches() {
    assert!(check_signature(|_| 0xff, b""));
}

#[test]
fn stops_reading_at_the_first_mismatch() {
    let mut reads = 0;
    let ok = check_signature(
        |i| {
            reads += 1;
            if i == 1 { 0 } else { b'A' }
        },
        b"AAB",
    );
    assert!(!ok);
    assert_eq!(reads, 2);
}

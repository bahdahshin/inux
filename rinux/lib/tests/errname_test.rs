// SPDX-License-Identifier: GPL-2.0-or-later
//! Host tests for `errname`. Linux has no KUnit file for it, so these check
//! the sign convention and the edges, then walk every table entry.

use rinux_lib::errname::errname;
use rinux_lib::errno::MAX_ERRNO;

#[test]
fn positive_and_negative_keep_their_sign() {
    assert_eq!(errname(5).unwrap().to_string(), "EIO");
    assert_eq!(errname(-5).unwrap().to_string(), "-EIO");
    assert_eq!(errname(512).unwrap().to_string(), "ERESTARTSYS");
    assert_eq!(errname(-512).unwrap().to_string(), "-ERESTARTSYS");
}

#[test]
fn zero_and_out_of_range_have_no_name() {
    assert!(errname(0).is_none());
    assert!(errname(MAX_ERRNO + 1).is_none());
    assert!(errname(-(MAX_ERRNO + 1)).is_none());
    assert!(errname(i32::MIN).is_none());
    assert!(errname(i32::MAX).is_none());
}

#[test]
fn unknown_numbers_have_no_name() {
    // In range but not in the arch-generic table.
    assert!(errname(-200).is_none());
}

#[test]
fn every_table_entry_round_trips() {
    let mut seen = 0;
    for n in 1..=MAX_ERRNO {
        let Some(name) = errname(n) else { continue };
        seen += 1;
        let text = name.to_string();
        assert!(!text.starts_with('-'), "{n}: {text}");
        assert_eq!(errname(-n).unwrap().to_string(), format!("-{text}"));
    }
    assert!(seen > 130, "only {seen} names found");
}

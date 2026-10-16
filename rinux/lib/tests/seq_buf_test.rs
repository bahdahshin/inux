// SPDX-License-Identifier: GPL-2.0-or-later
//! Host tests for `seq_buf`. Linux has no KUnit file for it, so these cover
//! the overflow rules and the formatted helpers.

use rinux_lib::seq_buf::{DumpPrefix, Overflow, SeqBuf};

#[test]
fn puts_and_putc_append() {
    let mut storage = [0xffu8; 16];
    let mut s = SeqBuf::new(&mut storage);
    assert_eq!(s.as_bytes(), b"");
    s.puts("ab").unwrap();
    s.putc(b'c').unwrap();
    assert_eq!(s.as_bytes(), b"abc");
    assert_eq!(s.buffer_left(), 13);
}

#[test]
fn a_write_that_leaves_no_room_for_nul_overflows() {
    let mut storage = [0u8; 4];
    let mut s = SeqBuf::new(&mut storage);
    s.puts("abc").unwrap(); // three bytes plus NUL fits exactly in four
    assert_eq!(s.puts("x"), Err(Overflow));
    assert!(s.has_overflowed());
    assert_eq!(s.used(), 4);
    assert_eq!(s.buffer_left(), 0);
    // Nothing more is accepted once overflowed.
    assert_eq!(s.putc(b'y'), Err(Overflow));
}

#[test]
fn putmem_hex_prints_groups_of_eight_little_endian() {
    let mut storage = [0u8; 64];
    let mut s = SeqBuf::new(&mut storage);
    s.putmem_hex(&[1, 2, 3]).unwrap();
    assert_eq!(s.as_bytes(), b"030201 ");

    let mut storage = [0u8; 64];
    let mut s = SeqBuf::new(&mut storage);
    s.putmem_hex(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10]).unwrap();
    assert_eq!(s.as_bytes(), b"0807060504030201 0a09 ");
}

#[test]
fn printf_appends_and_respects_size() {
    let mut storage = [0u8; 16];
    let mut s = SeqBuf::new(&mut storage);
    s.printf(format_args!("x={} y={:x}", 12, 255)).unwrap();
    assert_eq!(s.as_bytes(), b"x=12 y=ff");

    let mut storage = [0u8; 4];
    let mut s = SeqBuf::new(&mut storage);
    assert_eq!(s.printf(format_args!("{}", 12345)), Err(Overflow));
    assert!(s.has_overflowed());
}

#[test]
fn hex_dump_prints_one_line_per_row() {
    let mut storage = [0u8; 256];
    let mut s = SeqBuf::new(&mut storage);
    let data: Vec<u8> = (0..20).collect();
    s.hex_dump("", DumpPrefix::Offset, 16, 1, &data, false)
        .unwrap();
    let text = std::str::from_utf8(s.as_bytes()).unwrap();
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 2);
    assert!(
        lines[0].starts_with("00000000: 00 01 02 03"),
        "{}",
        lines[0]
    );
    assert!(
        lines[1].starts_with("00000010: 10 11 12 13"),
        "{}",
        lines[1]
    );
}

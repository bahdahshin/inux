// SPDX-License-Identifier: GPL-2.0
//! Host tests for `print_hex_dump()` (`lib/hexdump.c`) and the line sink in
//! `lib/src/printk.rs`. The row text comes from `hex_dump_to_buffer()`, which
//! has its own tests, so these check the rows, prefixes, levels and limits.

use rinux_lib::hexdump::{DUMP_LINE_MAX, DumpOpts, DumpPrefix, hex_dump_to_buffer, print_hex_dump};
use rinux_lib::printk::{LOGLEVEL_DEBUG, LOGLEVEL_INFO};

/// Runs `print_hex_dump()` and returns the `(level, line)` pairs it sent.
fn dump(level: i32, opts: DumpOpts<'_>, buf: &[u8]) -> Vec<(i32, String)> {
    let mut lines = Vec::new();
    print_hex_dump(level, opts, buf, &mut |lvl, line| {
        lines.push((lvl, String::from_utf8(line.to_vec()).unwrap()));
    });
    lines
}

/// The row text `hex_dump_to_buffer()` gives for `row`, as a `String`.
fn row_text(row: &[u8], rowsize: i32, groupsize: i32, ascii: bool) -> String {
    // Large enough for a 32-byte row with its ASCII column.
    let mut linebuf = [0u8; 256];
    hex_dump_to_buffer(row, rowsize, groupsize, &mut linebuf, ascii);
    let end = linebuf.iter().position(|&b| b == 0).unwrap();
    String::from_utf8(linebuf[..end].to_vec()).unwrap()
}

fn opts(prefix_type: DumpPrefix) -> DumpOpts<'static> {
    DumpOpts {
        prefix_str: b"hd: ",
        prefix_type,
        rowsize: 16,
        groupsize: 1,
        base: 0x1000,
        ascii: true,
    }
}

#[test]
fn empty_buffer_prints_nothing() {
    assert!(dump(LOGLEVEL_INFO, opts(DumpPrefix::None), &[]).is_empty());
}

#[test]
fn rows_split_at_rowsize_and_the_last_row_is_short() {
    let data: Vec<u8> = (0..20).collect();
    let lines = dump(LOGLEVEL_INFO, opts(DumpPrefix::None), &data);

    assert_eq!(lines.len(), 2);
    assert_eq!(
        lines[0].1,
        format!("hd: {}", row_text(&data[..16], 16, 1, true))
    );
    assert_eq!(
        lines[1].1,
        format!("hd: {}", row_text(&data[16..], 16, 1, true))
    );
}

#[test]
fn none_prefix_is_only_the_prefix_string() {
    let lines = dump(LOGLEVEL_INFO, opts(DumpPrefix::None), b"A");
    assert!(lines[0].1.starts_with("hd: 41 "));
}

#[test]
fn address_prefix_shows_the_row_address_and_ignores_the_prefix_string() {
    let data = [0u8; 20];
    let lines = dump(LOGLEVEL_INFO, opts(DumpPrefix::Address), &data);

    assert_eq!(
        lines[0].1,
        format!("0x0000000000001000: {}", row_text(&data[..16], 16, 1, true))
    );
    assert_eq!(
        lines[1].1,
        format!("0x0000000000001010: {}", row_text(&data[16..], 16, 1, true))
    );
}

#[test]
fn offset_prefix_is_the_prefix_string_then_the_row_offset() {
    let data = [0u8; 20];
    let lines = dump(LOGLEVEL_INFO, opts(DumpPrefix::Offset), &data);

    assert!(lines[0].1.starts_with("hd: 00000000: "));
    assert!(lines[1].1.starts_with("hd: 00000010: "));
}

#[test]
fn every_line_gets_the_requested_level() {
    let lines = dump(LOGLEVEL_DEBUG, opts(DumpPrefix::None), &[1, 2, 3]);
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].0, LOGLEVEL_DEBUG);
}

#[test]
fn bad_rowsize_means_sixteen_bytes() {
    let data: Vec<u8> = (0..20).collect();
    let mut o = opts(DumpPrefix::None);
    o.rowsize = 7;
    let lines = dump(LOGLEVEL_INFO, o, &data);
    assert_eq!(lines.len(), 2);
    assert_eq!(
        lines[0].1,
        format!("hd: {}", row_text(&data[..16], 16, 1, true))
    );
}

#[test]
fn rowsize_thirty_two_gives_wider_rows() {
    let data: Vec<u8> = (0..40).collect();
    let mut o = opts(DumpPrefix::None);
    o.rowsize = 32;
    let lines = dump(LOGLEVEL_INFO, o, &data);
    assert_eq!(lines.len(), 2);
    assert_eq!(
        lines[0].1,
        format!("hd: {}", row_text(&data[..32], 32, 1, true))
    );
}

#[test]
fn a_long_prefix_is_cut_to_the_line_limit() {
    let prefix = vec![b'p'; DUMP_LINE_MAX + 100];
    let o = DumpOpts {
        prefix_str: &prefix,
        ..opts(DumpPrefix::None)
    };
    let lines = dump(LOGLEVEL_INFO, o, b"x");
    assert_eq!(lines[0].1.len(), DUMP_LINE_MAX);
    assert!(lines[0].1.bytes().all(|b| b == b'p'));
}

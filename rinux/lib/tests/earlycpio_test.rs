// SPDX-License-Identifier: GPL-2.0-or-later
//! Host tests for `find_cpio_data`, over archives built in the test with the
//! `newc` layout. Linux has no KUnit file for `earlycpio.c`.

use rinux_lib::earlycpio::find_cpio_data;

/// One `newc` entry: 110-byte header, name, name padding, data, data padding.
fn entry(name: &str, mode: u32, body: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let namesize = name.len() + 1;
    let fields = [
        0x0007_0701,
        1,
        mode,
        0,
        0,
        1,
        0,
        body.len() as u32,
        0,
        0,
        0,
        0,
        namesize as u32,
        0,
    ];
    out.extend_from_slice(format!("{:06x}", fields[0]).as_bytes());
    for f in &fields[1..] {
        out.extend_from_slice(format!("{f:08x}").as_bytes());
    }
    out.extend_from_slice(name.as_bytes());
    out.push(0);
    while out.len() % 4 != 0 {
        out.push(0);
    }
    out.extend_from_slice(body);
    while out.len() % 4 != 0 {
        out.push(0);
    }
    out
}

fn archive() -> Vec<u8> {
    let mut a = entry("kernel/a.bin", 0o100644, b"AAAA");
    a.extend(entry("kernel/b.bin", 0o100644, b"BBBBBBB"));
    a.extend(entry("TRAILER!!!", 0, b""));
    a
}

#[test]
fn finds_a_file_by_prefix_and_returns_its_data() {
    let a = archive();
    let found = find_cpio_data(b"kernel/b", &a).unwrap();
    assert_eq!(found.data, b"BBBBBBB");
    assert_eq!(found.name, b".bin");
}

#[test]
fn next_offset_points_past_the_found_entry() {
    let a = archive();
    let first = find_cpio_data(b"kernel/a", &a).unwrap();
    let second = find_cpio_data(b"kernel/b", &a[first.next_off..]).unwrap();
    assert_eq!(second.data, b"BBBBBBB");
}

#[test]
fn a_missing_name_gives_none() {
    assert!(find_cpio_data(b"nothere", &archive()).is_none());
}

#[test]
fn a_bad_magic_stops_the_search() {
    let mut a = entry("x", 0o100644, b"data");
    a[0] = b'9';
    assert!(find_cpio_data(b"x", &a).is_none());
}

#[test]
fn a_directory_entry_is_not_a_file() {
    let a = entry("kernel", 0o040755, b"");
    assert!(find_cpio_data(b"kernel", &a).is_none());
}

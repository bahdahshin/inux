// SPDX-License-Identifier: GPL-2.0-only
//! Port of `lib/test_hexdump.c`.
//!
//! The C test picks row sizes and lengths at random; this one runs every
//! combination instead.

use rinux_lib::hexdump::*;

const DATA_B: [u8; 32] = [
    0xbe, 0x32, 0xdb, 0x7b, 0x0a, 0x18, 0x93, 0xb2, // 00 - 07
    0x70, 0xba, 0xc4, 0x24, 0x7d, 0x83, 0x34, 0x9b, // 08 - 0f
    0xa6, 0x9c, 0x31, 0xad, 0x9c, 0x0f, 0xac, 0xe9, // 10 - 17
    0x4c, 0xd1, 0x19, 0x99, 0x43, 0xb1, 0xaf, 0x0c, // 18 - 1f
];

const DATA_A: &[u8] = b".2.{....p..$}.4...1.....L...C...";

const TEST_DATA_1: [&str; 32] = [
    "be", "32", "db", "7b", "0a", "18", "93", "b2", "70", "ba", "c4", "24", "7d", "83", "34", "9b",
    "a6", "9c", "31", "ad", "9c", "0f", "ac", "e9", "4c", "d1", "19", "99", "43", "b1", "af", "0c",
];
const TEST_DATA_2_LE: [&str; 16] = [
    "32be", "7bdb", "180a", "b293", "ba70", "24c4", "837d", "9b34", "9ca6", "ad31", "0f9c", "e9ac",
    "d14c", "9919", "b143", "0caf",
];
const TEST_DATA_2_BE: [&str; 16] = [
    "be32", "db7b", "0a18", "93b2", "70ba", "c424", "7d83", "349b", "a69c", "31ad", "9c0f", "ace9",
    "4cd1", "1999", "43b1", "af0c",
];
const TEST_DATA_4_LE: [&str; 8] = [
    "7bdb32be", "b293180a", "24c4ba70", "9b34837d", "ad319ca6", "e9ac0f9c", "9919d14c", "0cafb143",
];
const TEST_DATA_4_BE: [&str; 8] = [
    "be32db7b", "0a1893b2", "70bac424", "7d83349b", "a69c31ad", "9c0face9", "4cd11999", "43b1af0c",
];
const TEST_DATA_8_LE: [&str; 4] = [
    "b293180a7bdb32be",
    "9b34837d24c4ba70",
    "e9ac0f9cad319ca6",
    "0cafb1439919d14c",
];
const TEST_DATA_8_BE: [&str; 4] = [
    "be32db7b0a1893b2",
    "70bac4247d83349b",
    "a69c31ad9c0face9",
    "4cd1199943b1af0c",
];

const FILL_CHAR: u8 = b'#';
const TEST_HEXDUMP_BUF_SIZE: usize = 32 * 3 + 2 + 32 + 1;

/// `test_hexdump_prepare_test()`: builds the expected line by hand.
fn prepare_test(len: usize, rowsize: i32, groupsize: i32, test: &mut [u8], ascii: bool) {
    let rs = if rowsize != 16 && rowsize != 32 {
        16
    } else {
        rowsize as usize
    };
    let l = len.min(rs);
    let mut gs = groupsize as usize;
    if !gs.is_power_of_two() || gs > 8 || len % gs != 0 {
        gs = 1;
    }
    let be = cfg!(target_endian = "big");
    let result: &[&str] = match gs {
        8 if be => &TEST_DATA_8_BE,
        8 => &TEST_DATA_8_LE,
        4 if be => &TEST_DATA_4_BE,
        4 => &TEST_DATA_4_LE,
        2 if be => &TEST_DATA_2_BE,
        2 => &TEST_DATA_2_LE,
        _ => &TEST_DATA_1,
    };

    // Hex dump.
    let mut p = 0;
    for q in &result[..l / gs] {
        test[p..p + q.len()].copy_from_slice(q.as_bytes());
        p += q.len();
        test[p] = b' ';
        p += 1;
    }
    if l / gs != 0 {
        p -= 1;
    }

    // ASCII part.
    if ascii {
        loop {
            test[p] = b' ';
            p += 1;
            if p > rs * 2 + rs / gs {
                break;
            }
        }
        test[p..p + l].copy_from_slice(&DATA_A[..l]);
        p += l;
    }
    test[p] = 0;
}

/// `test_hexdump()`.
fn test_hexdump(len: usize, rowsize: i32, groupsize: i32, ascii: bool) {
    let mut real = [FILL_CHAR; TEST_HEXDUMP_BUF_SIZE];
    hex_dump_to_buffer(&DATA_B[..len], rowsize, groupsize, &mut real, ascii);

    let mut test = [FILL_CHAR; TEST_HEXDUMP_BUF_SIZE];
    prepare_test(len, rowsize, groupsize, &mut test, ascii);

    assert_eq!(
        String::from_utf8_lossy(&real),
        String::from_utf8_lossy(&test),
        "len {len} row {rowsize} group {groupsize} ascii {ascii}"
    );
}

/// `test_hexdump_overflow()`: output truncated to `buflen` bytes.
fn test_hexdump_overflow(buflen: usize, len: usize, rs: i32, gs: i32, ascii: bool) {
    let mut buf = [FILL_CHAR; TEST_HEXDUMP_BUF_SIZE];
    let r = hex_dump_to_buffer(&DATA_B[..len], rs, gs, &mut buf[..buflen], ascii);

    // The caller provides a length that is a multiple of the group size.
    let (rsu, gsu) = (rs as usize, gs as usize);
    let ae = rsu * 2 + rsu / gsu + 1 + len;
    let he = ((gsu * 2 + 1) * len / gsu) as i32 - 1;
    let e = if ascii { ae as i32 } else { he };

    let f = ((e + 1) as usize).min(buflen);
    let mut test = [FILL_CHAR; TEST_HEXDUMP_BUF_SIZE];
    if buflen != 0 {
        prepare_test(len, rs, gs, &mut test, ascii);
        test[f - 1] = 0;
    }
    test[f..].fill(FILL_CHAR);

    assert_eq!(
        r, e,
        "len {len} buflen {buflen} row {rs} group {gs} ascii {ascii}"
    );
    assert_eq!(
        String::from_utf8_lossy(&buf),
        String::from_utf8_lossy(&test),
        "len {len} buflen {buflen} row {rs} group {gs} ascii {ascii}"
    );
}

#[test]
fn hexdump_lines() {
    for rowsize in [16, 32] {
        for len in 1..=rowsize.min(DATA_B.len() as i32) as usize {
            for ascii in [false, true] {
                for groupsize in [4, 2, 8, 1] {
                    test_hexdump(len, rowsize, groupsize, ascii);
                }
            }
        }
    }
}

#[test]
fn hexdump_overflow() {
    for ascii in [false, true] {
        for buflen in 0..=TEST_HEXDUMP_BUF_SIZE {
            for rs in [16, 32] {
                for i in 0..4 {
                    let gs = 1 << i;
                    for len in (gs as usize..rs as usize + gs as usize).step_by(gs as usize) {
                        let len = len.min(rs as usize) / gs as usize * gs as usize;
                        test_hexdump_overflow(buflen, len, rs, gs, ascii);
                    }
                }
            }
        }
    }
}

// Not in the C test.

#[test]
fn hex_conversions() {
    for c in 0u8..=255 {
        let want = (c as char).to_digit(16).map_or(-1, |d| d as i32);
        assert_eq!(hex_to_bin(c), want, "{c:#x}");
    }

    let mut bin = [0u8; 4];
    assert_eq!(hex2bin(&mut bin, b"deadBEEF"), Ok(()));
    assert_eq!(bin, [0xde, 0xad, 0xbe, 0xef]);
    assert!(hex2bin(&mut bin, b"deadBEEG").is_err());
    assert!(hex2bin(&mut bin, b"dead").is_err());

    let mut hex = [0u8; 8];
    assert_eq!(bin2hex(&mut hex, &[0xde, 0xad, 0xbe, 0xef]), 8);
    assert_eq!(&hex, b"deadbeef");

    let mut two = [0u8; 2];
    hex_byte_pack_upper(&mut two, 0xab);
    assert_eq!(&two, b"AB");
}

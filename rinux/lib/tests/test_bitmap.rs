// SPDX-License-Identifier: GPL-2.0-only
//! Port of `lib/test_bitmap.c`.
//!
//! `test_bitmap_read_perf()` and `test_bitmap_write_perf()` only time
//! loops and are not ported. `find_random_bit()` in `test_zero_nbits()`
//! waits for the random number generator.

use rinux_lib::bitmap::*;
use rinux_lib::bitops::{
    BITS_PER_LONG, assign_bit, bit, change_bit, genmask, genmask_ull, set_bit, test_bit,
};
use rinux_lib::errno::{EINVAL, ENOMEM, EOVERFLOW, ERANGE, Errno};
use rinux_lib::find_bit::*;
use rinux_lib::{scnprintf, snprintf};

include!("data/test_bitmap_strings.rs");

const PAGE_SIZE: usize = 4096;

const EXP1: [u64; 15] = [
    1,
    2,
    0x0000ffff,
    0xffff0000,
    0x55555555,
    0xaaaaaaaa,
    0x11111111,
    0x22222222,
    0xffffffff,
    0xfffffffe,
    0x3333333311111111,
    0xffffffff77777777,
    0,
    0x00008000,
    0x80000000,
];
const EXP1_IN_BITS: usize = EXP1.len() * 64;

const EXP2: [u64; 2] = [0x3333333311111111, 0xffffffff77777777];
const EXP2_IN_BITS: usize = EXP2.len() * 64;

/// Fibonacci sequence.
const EXP2_TO_EXP3_MASK: [u64; 1] = [0x008000020020212e];
/// `(exp2[0] & ~exp2_to_exp3_mask) | (exp2[1] & exp2_to_exp3_mask)`.
const EXP3_0_1: [u64; 1] = [0x33b3333311313137];
/// `(exp2[1] & ~exp2_to_exp3_mask) | (exp2[0] & exp2_to_exp3_mask)`.
const EXP3_1_0: [u64; 1] = [0xff7fffff77575751];

/// `__check_eq_bitmap()`.
#[track_caller]
fn expect_eq_bitmap(exp_bmap: &[u64], bmap: &[u64], nbits: usize) {
    assert!(
        bitmap_equal(exp_bmap, bmap, nbits),
        "bitmaps contents differ: expected \"{}\", got \"{}\"",
        pbl(exp_bmap, nbits),
        pbl(bmap, nbits)
    );
}

/// `"%*pbl"` of a bitmap.
fn pbl(bitmap: &[u64], nbits: usize) -> String {
    let mut pbl_buffer = vec![0u8; PAGE_SIZE];
    let n = snprintf!(&mut pbl_buffer, "%*pbl", nbits, bitmap);
    String::from_utf8_lossy(&pbl_buffer[..n.min(PAGE_SIZE - 1)]).into_owned()
}

/// `__check_eq_pbl()`.
#[track_caller]
fn expect_eq_pbl(expected_pbl: &str, bitmap: &[u64], nbits: usize) {
    assert_eq!(pbl(bitmap, nbits), expected_pbl);
}

#[test]
fn test_zero_clear() {
    let mut bmap = [0u64; 16];

    // Known way to set all bits.
    bmap.fill(!0);

    expect_eq_pbl("0-22", &bmap, 23);
    expect_eq_pbl("0-1023", &bmap, 1024);

    // Single-word bitmaps.
    bitmap_clear(&mut bmap, 0, 9);
    expect_eq_pbl("9-1023", &bmap, 1024);

    bitmap_zero(&mut bmap, 35);
    expect_eq_pbl("64-1023", &bmap, 1024);

    // Cross boundaries operations.
    bitmap_clear(&mut bmap, 79, 19);
    expect_eq_pbl("64-78,98-1023", &bmap, 1024);

    bitmap_zero(&mut bmap, 115);
    expect_eq_pbl("128-1023", &bmap, 1024);

    // Zeroing entire area.
    bitmap_zero(&mut bmap, 1024);
    expect_eq_pbl("", &bmap, 1024);
}

#[test]
fn test_find_nth_bit() {
    let mut bmap = [0u64; 3];

    bitmap_zero(&mut bmap, 64 * 3);
    for b in [10, 20, 30, 40, 50, 60, 80, 123] {
        set_bit(b, &mut bmap);
    }

    assert_eq!(10, find_nth_bit(&bmap, 64 * 3, 0));
    assert_eq!(20, find_nth_bit(&bmap, 64 * 3, 1));
    assert_eq!(30, find_nth_bit(&bmap, 64 * 3, 2));
    assert_eq!(40, find_nth_bit(&bmap, 64 * 3, 3));
    assert_eq!(50, find_nth_bit(&bmap, 64 * 3, 4));
    assert_eq!(60, find_nth_bit(&bmap, 64 * 3, 5));
    assert_eq!(80, find_nth_bit(&bmap, 64 * 3, 6));
    assert_eq!(123, find_nth_bit(&bmap, 64 * 3, 7));
    assert!(find_nth_bit(&bmap, 64 * 3, 8) >= 64 * 3);

    assert_eq!(10, find_nth_bit(&bmap, 64 * 3 - 1, 0));
    assert_eq!(20, find_nth_bit(&bmap, 64 * 3 - 1, 1));
    assert_eq!(30, find_nth_bit(&bmap, 64 * 3 - 1, 2));
    assert_eq!(40, find_nth_bit(&bmap, 64 * 3 - 1, 3));
    assert_eq!(50, find_nth_bit(&bmap, 64 * 3 - 1, 4));
    assert_eq!(60, find_nth_bit(&bmap, 64 * 3 - 1, 5));
    assert_eq!(80, find_nth_bit(&bmap, 64 * 3 - 1, 6));
    assert_eq!(123, find_nth_bit(&bmap, 64 * 3 - 1, 7));
    assert!(find_nth_bit(&bmap, 64 * 3 - 1, 8) >= 64 * 3 - 1);

    for (cnt, b) in for_each_set_bit(&EXP1, EXP1_IN_BITS).enumerate() {
        assert_eq!(find_nth_bit(&EXP1, EXP1_IN_BITS, cnt), b);
    }
}

#[test]
fn test_bitmap_find_next_zero_area_off() {
    let mut bmap = [0u64; 3];

    bitmap_set(&mut bmap, 0, 192);

    bitmap_clear(&mut bmap, 0, 8);
    rinux_lib::bitops::clear_bit(50, &mut bmap);
    bitmap_clear(&mut bmap, 60, 18);
    set_bit(69, &mut bmap);
    rinux_lib::bitops::clear_bit(80, &mut bmap);
    bitmap_clear(&mut bmap, 100, 10);
    rinux_lib::bitops::clear_bit(120, &mut bmap);
    bitmap_clear(&mut bmap, 145, 8);
    bitmap_clear(&mut bmap, 160, 32);

    assert_eq!(0, bitmap_find_next_zero_area_off(&bmap, 192, 0, 8, 0, 0));
    assert_eq!(0, bitmap_find_next_zero_area_off(&bmap, 192, 0, 8, 3, 0));
    assert_eq!(163, bitmap_find_next_zero_area_off(&bmap, 192, 0, 8, 3, 1));
    assert_eq!(60, bitmap_find_next_zero_area_off(&bmap, 192, 1, 8, 0, 0));
    assert_eq!(160, bitmap_find_next_zero_area_off(&bmap, 192, 1, 8, 7, 0));
    assert_eq!(60, bitmap_find_next_zero_area_off(&bmap, 192, 1, 8, 7, 4));
    assert_eq!(100, bitmap_find_next_zero_area_off(&bmap, 192, 0, 10, 0, 0));
    assert_eq!(160, bitmap_find_next_zero_area_off(&bmap, 192, 0, 32, 0, 0));
    assert!(bitmap_find_next_zero_area_off(&bmap, 192, 0, 33, 0, 0) >= 192);
}

#[test]
fn test_fill_set() {
    let mut bmap = [0u64; 16];

    // Known way to clear all bits.
    bmap.fill(0);

    expect_eq_pbl("", &bmap, 23);
    expect_eq_pbl("", &bmap, 1024);

    // Single-word bitmaps.
    bitmap_set(&mut bmap, 0, 9);
    expect_eq_pbl("0-8", &bmap, 1024);

    bitmap_fill(&mut bmap, 35);
    expect_eq_pbl("0-63", &bmap, 1024);

    // Cross boundaries operations.
    bitmap_set(&mut bmap, 79, 19);
    expect_eq_pbl("0-63,79-97", &bmap, 1024);

    bitmap_fill(&mut bmap, 115);
    expect_eq_pbl("0-127", &bmap, 1024);

    // Zeroing entire area.
    bitmap_fill(&mut bmap, 1024);
    expect_eq_pbl("0-1023", &bmap, 1024);
}

#[test]
fn test_copy() {
    let mut bmap1 = [0u64; 16];
    let mut bmap2 = [0u64; 16];

    bitmap_zero(&mut bmap1, 1024);
    bitmap_zero(&mut bmap2, 1024);

    // Single-word bitmaps.
    bitmap_set(&mut bmap1, 0, 19);
    bitmap_copy(&mut bmap2, &bmap1, 23);
    expect_eq_pbl("0-18", &bmap2, 1024);

    bitmap_set(&mut bmap2, 0, 23);
    bitmap_copy(&mut bmap2, &bmap1, 23);
    expect_eq_pbl("0-18", &bmap2, 1024);

    // Multi-word bitmaps.
    bitmap_set(&mut bmap1, 0, 109);
    bitmap_copy(&mut bmap2, &bmap1, 1024);
    expect_eq_pbl("0-108", &bmap2, 1024);

    bitmap_fill(&mut bmap2, 1024);
    bitmap_copy(&mut bmap2, &bmap1, 1024);
    expect_eq_pbl("0-108", &bmap2, 1024);

    // The following tests assume a 32- or 64-bit arch (even 128b if we
    // care).

    bitmap_fill(&mut bmap2, 1024);
    bitmap_copy(&mut bmap2, &bmap1, 109); // ... but 0-padded til word length
    expect_eq_pbl("0-108,128-1023", &bmap2, 1024);

    bitmap_fill(&mut bmap2, 1024);
    bitmap_copy(&mut bmap2, &bmap1, 97); // ... but aligned on word length
    expect_eq_pbl("0-108,128-1023", &bmap2, 1024);
}

#[test]
fn test_bitmap_region() {
    let mut bmap = [0u64; 16];

    bitmap_zero(&mut bmap, 1000);

    for order in 0..10 {
        let pos = bitmap_find_free_region(&mut bmap, 1000, order);
        if order == 0 {
            assert_eq!(pos, Ok(0));
        } else {
            assert_eq!(
                pos,
                if order < 9 {
                    Ok(bit(order) as usize)
                } else {
                    Err(ENOMEM)
                }
            );
        }
    }

    bitmap_release_region(&mut bmap, 0, 0);
    for order in 1..9 {
        bitmap_release_region(&mut bmap, bit(order) as usize, order);
    }

    assert_eq!(bitmap_weight(&bmap, 1000), 0);
}

#[test]
fn test_replace() {
    let nbits = 64;
    let nlongs = nbits / BITS_PER_LONG;
    let mut bmap = [0u64; 16];

    assert!(EXP2_IN_BITS >= nbits * 2);

    bitmap_zero(&mut bmap, 1024);
    bitmap_replace(
        &mut bmap,
        &EXP2[0..],
        &EXP2[nlongs..],
        &EXP2_TO_EXP3_MASK,
        nbits,
    );
    expect_eq_bitmap(&bmap, &EXP3_0_1, nbits);

    bitmap_zero(&mut bmap, 1024);
    bitmap_replace(
        &mut bmap,
        &EXP2[nlongs..],
        &EXP2[0..],
        &EXP2_TO_EXP3_MASK,
        nbits,
    );
    expect_eq_bitmap(&bmap, &EXP3_1_0, nbits);

    bitmap_fill(&mut bmap, 1024);
    bitmap_replace(
        &mut bmap,
        &EXP2[0..],
        &EXP2[nlongs..],
        &EXP2_TO_EXP3_MASK,
        nbits,
    );
    expect_eq_bitmap(&bmap, &EXP3_0_1, nbits);

    bitmap_fill(&mut bmap, 1024);
    bitmap_replace(
        &mut bmap,
        &EXP2[nlongs..],
        &EXP2[0..],
        &EXP2_TO_EXP3_MASK,
        nbits,
    );
    expect_eq_bitmap(&bmap, &EXP3_1_0, nbits);
}

const SG_MASK: [u64; 2] = [0x000000000000035a, 0];
const SG_SRC: [u64; 2] = [0x0000000000000667, 0];
const SG_GATHER_EXP: [u64; 2] = [0x0000000000000029, 0];
const SG_SCATTER_EXP: [u64; 2] = [0x000000000000021a, 0];

#[test]
fn test_bitmap_sg() {
    let nbits = 64;
    let mut bmap_gather = [0u64; 2];
    let mut bmap_scatter = [0u64; 2];
    let mut bmap_tmp = [0u64; 2];
    let mut bmap_res = [0u64; 2];

    // Simple gather call.
    bitmap_zero(&mut bmap_gather, 100);
    bitmap_gather(&mut bmap_gather, &SG_SRC, &SG_MASK, nbits);
    expect_eq_bitmap(&SG_GATHER_EXP, &bmap_gather, 100);

    // Simple scatter call.
    bitmap_zero(&mut bmap_scatter, 100);
    bitmap_scatter(&mut bmap_scatter, &SG_SRC, &SG_MASK, nbits);
    expect_eq_bitmap(&SG_SCATTER_EXP, &bmap_scatter, 100);

    // Scatter/gather relationship.
    bitmap_zero(&mut bmap_tmp, 100);
    bitmap_zero(&mut bmap_res, 100);
    bitmap_gather(&mut bmap_tmp, &bmap_scatter, &SG_MASK, nbits);
    bitmap_scatter(&mut bmap_res, &bmap_tmp, &SG_MASK, nbits);
    expect_eq_bitmap(&bmap_scatter, &bmap_res, 100);
}

/// `struct test_bitmap_parselist`: result, input, expected bitmap (an
/// array and the index it starts at), bits, flags.
struct ParseTest {
    errno: Option<Errno>,
    input: &'static str,
    expected: Option<(&'static [u64], usize)>,
    nbits: usize,
    flags: u32,
}

const PARSE_TIME: u32 = 0x1;
const NO_LEN: u32 = 0x2;

const fn ok(
    input: &'static str,
    expected: Option<(&'static [u64], usize)>,
    nbits: usize,
    flags: u32,
) -> ParseTest {
    ParseTest {
        errno: None,
        input,
        expected,
        nbits,
        flags,
    }
}

const fn err(errno: Errno, input: &'static str, nbits: usize) -> ParseTest {
    ParseTest {
        errno: Some(errno),
        input,
        expected: None,
        nbits,
        flags: 0,
    }
}

const fn e1(i: usize) -> Option<(&'static [u64], usize)> {
    Some((&EXP1, i))
}

/// `step` is 1 on 64-bit machines.
const PARSELIST_TESTS: &[ParseTest] = &[
    ok("0", e1(0), 8, 0),
    ok("1", e1(1), 8, 0),
    ok("0-15", e1(2), 32, 0),
    ok("16-31", e1(3), 32, 0),
    ok("0-31:1/2", e1(4), 32, 0),
    ok("1-31:1/2", e1(5), 32, 0),
    ok("0-31:1/4", e1(6), 32, 0),
    ok("1-31:1/4", e1(7), 32, 0),
    ok("0-31:4/4", e1(8), 32, 0),
    ok("1-31:4/4", e1(9), 32, 0),
    ok("0-31:1/4,32-63:2/4", e1(10), 64, 0),
    ok("0-31:3/4,32-63:4/4", e1(11), 64, 0),
    ok("  ,,  0-31:3/4  ,, 32-63:4/4  ,,  ", e1(11), 64, 0),
    ok(
        "0-31:1/4,32-63:2/4,64-95:3/4,96-127:4/4",
        Some((&EXP2, 0)),
        128,
        0,
    ),
    ok("0-2047:128/256", None, 2048, PARSE_TIME),
    ok("", e1(12), 8, 0),
    ok("\n", e1(12), 8, 0),
    ok(",,  ,,  , ,  ,", e1(12), 8, 0),
    ok(" ,  ,,  , ,   ", e1(12), 8, 0),
    ok(" ,  ,,  , ,   \n", e1(12), 8, 0),
    ok("0-0", e1(0), 32, 0),
    ok("1-1", e1(1), 32, 0),
    ok("15-15", e1(13), 32, 0),
    ok("31-31", e1(14), 32, 0),
    ok("0-0:0/1", e1(12), 32, 0),
    ok("0-0:1/1", e1(0), 32, 0),
    ok("0-0:1/31", e1(0), 32, 0),
    ok("0-0:31/31", e1(0), 32, 0),
    ok("1-1:1/1", e1(1), 32, 0),
    ok("0-15:16/31", e1(2), 32, 0),
    ok("15-15:1/2", e1(13), 32, 0),
    ok("15-15:31/31", e1(13), 32, 0),
    ok("15-31:1/31", e1(13), 32, 0),
    ok("16-31:16/31", e1(3), 32, 0),
    ok("31-31:31/31", e1(14), 32, 0),
    ok("N-N", e1(14), 32, 0),
    ok("0-0:1/N", e1(0), 32, 0),
    ok("0-0:N/N", e1(0), 32, 0),
    ok("0-15:16/N", e1(2), 32, 0),
    ok("15-15:N/N", e1(13), 32, 0),
    ok("15-N:1/N", e1(13), 32, 0),
    ok("16-N:16/N", e1(3), 32, 0),
    ok("N-N:N/N", e1(14), 32, 0),
    ok("0-N:1/3,1-N:1/3,2-N:1/3", e1(8), 32, 0),
    ok("0-31:1/3,1-31:1/3,2-31:1/3", e1(8), 32, 0),
    ok("1-10:8/12,8-31:24/29,0-31:0/3", e1(9), 32, 0),
    ok("all", e1(8), 32, 0),
    ok("0, 1, all,  ", e1(8), 32, 0),
    ok("all:1/2", e1(4), 32, 0),
    ok("ALL:1/2", e1(4), 32, 0),
    err(EINVAL, "al", 8),
    err(EINVAL, "alll", 8),
    err(EINVAL, "-1", 8),
    err(EINVAL, "-0", 8),
    err(EINVAL, "10-1", 8),
    err(ERANGE, "8-8", 8),
    err(ERANGE, "0-31", 8),
    err(EINVAL, "0-31:", 32),
    err(EINVAL, "0-31:0", 32),
    err(EINVAL, "0-31:0/", 32),
    err(EINVAL, "0-31:0/0", 32),
    err(EINVAL, "0-31:1/0", 32),
    err(EINVAL, "0-31:10/1", 32),
    err(EOVERFLOW, "0-98765432123456789:10/1", 8),
    err(EINVAL, "a-31", 8),
    err(EINVAL, "0-a1", 8),
    err(EINVAL, "a-31:10/1", 8),
    err(EINVAL, "0-31:a/1", 8),
    err(EINVAL, "0-\n", 8),
];

fn check_parse_result(
    kind: &str,
    i: usize,
    t: &ParseTest,
    result: Result<(), Errno>,
    bmap: &[u64],
) {
    assert_eq!(result.err(), t.errno, "{kind}: {i}: input is {:?}", t.input);
    if result.is_ok() {
        if let Some((arr, idx)) = t.expected {
            assert!(
                __bitmap_equal(bmap, &arr[idx..], t.nbits),
                "{kind}: {i}: input is {:?}, result is {:#x}, expected {:#x}",
                t.input,
                bmap[0],
                arr[idx]
            );
        }
    }
}

#[test]
fn test_bitmap_parselist() {
    let mut bmap = [0u64; 32];

    for (i, t) in PARSELIST_TESTS.iter().enumerate() {
        let result = bitmap_parselist(t.input.as_bytes(), &mut bmap, t.nbits);
        check_parse_result("parselist", i, t, result, &bmap);
        // PARSE_TIME entries only print their run time in C.
        let _ = t.flags & PARSE_TIME;
    }
}

#[test]
fn test_bitmap_printlist() {
    let bmap = vec![!0u64; PAGE_SIZE / 8];
    let mut buf = vec![0u8; PAGE_SIZE];
    let mut expected = [0u8; 256];

    let slen = snprintf!(&mut expected, "0-%ld", PAGE_SIZE * 8 - 1);

    let ret = scnprintf!(&mut buf, "%*pbl", PAGE_SIZE * 8, &bmap[..]);
    assert_eq!(
        ret, slen,
        "scnprintf(\"%*pbl\"): result is {ret}, expected {slen}"
    );
    assert_eq!(&buf[..slen], &expected[..slen]);
}

const PARSE_TEST: [u64; 4] = [0, 1, 0xdeadbeef, 0x100000000];

const PARSE_TEST2: [u64; 6] = [
    0x100000000,
    0xdeadbeef,
    0x100000000,
    0xbaadf00ddeadbeef,
    0x100000000,
    0x0badf00ddeadbeef,
];

const PARSE_TESTS: &[ParseTest] = &[
    ok("", Some((&PARSE_TEST, 0)), 32, 0),
    ok(" ", Some((&PARSE_TEST, 0)), 32, 0),
    ok("0", Some((&PARSE_TEST, 0)), 32, 0),
    ok("0\n", Some((&PARSE_TEST, 0)), 32, 0),
    ok("1", Some((&PARSE_TEST, 1)), 32, 0),
    ok("deadbeef", Some((&PARSE_TEST, 2)), 32, 0),
    ok("1,0", Some((&PARSE_TEST, 3)), 33, 0),
    ok("deadbeef,\n,0,1", Some((&PARSE_TEST, 2)), 96, 0),
    ok("deadbeef,1,0", Some((&PARSE_TEST2, 0)), 96, 0),
    ok("baadf00d,deadbeef,1,0", Some((&PARSE_TEST2, 2)), 128, 0),
    ok("badf00d,deadbeef,1,0", Some((&PARSE_TEST2, 4)), 124, 0),
    ok("badf00d,deadbeef,1,0", Some((&PARSE_TEST2, 4)), 124, NO_LEN),
    ok("  badf00d,deadbeef,1,0  ", Some((&PARSE_TEST2, 4)), 124, 0),
    ok(
        " , badf00d,deadbeef,1,0 , ",
        Some((&PARSE_TEST2, 4)),
        124,
        0,
    ),
    ok(
        " , badf00d, ,, ,,deadbeef,1,0 , ",
        Some((&PARSE_TEST2, 4)),
        124,
        0,
    ),
    err(EINVAL, "goodfood,deadbeef,1,0", 128),
    err(EOVERFLOW, "3,0", 33),
    err(EOVERFLOW, "123badf00d,deadbeef,1,0", 128),
    err(EOVERFLOW, "badf00d,deadbeef,1,0", 90),
    err(EOVERFLOW, "fbadf00d,deadbeef,1,0", 95),
    err(EOVERFLOW, "badf00d,deadbeef,1,0", 100),
];

#[test]
fn test_bitmap_parse() {
    let mut bmap = [0u64; 32];

    for (i, t) in PARSE_TESTS.iter().enumerate() {
        let len = if t.flags & NO_LEN != 0 {
            u32::MAX as usize
        } else {
            t.input.len()
        };
        let result = bitmap_parse(t.input.as_bytes(), len, &mut bmap, t.nbits);
        check_parse_result("parse", i, t, result, &bmap);
    }
}

#[test]
fn test_bitmap_arr32() {
    let mut arr = [0xa5a5a5a5u32; EXP1_IN_BITS / 32];
    let mut bmap2 = [0u64; EXP1_IN_BITS / 64];

    for nbits in 1..EXP1_IN_BITS {
        bitmap_to_arr32(&mut arr, &EXP1, nbits);
        bitmap_from_arr32(&mut bmap2, &arr, nbits);
        expect_eq_bitmap(&bmap2, &EXP1, nbits);

        let round_up = nbits.next_multiple_of(BITS_PER_LONG);
        let next_bit = find_next_bit(&bmap2, round_up, nbits);
        assert!(
            next_bit >= round_up,
            "bitmap_copy_arr32(nbits == {nbits}: tail is not safely cleared: {next_bit}"
        );

        if nbits < EXP1_IN_BITS - 32 {
            assert_eq!(arr[nbits.div_ceil(32)], 0xa5a5a5a5);
        }
    }
}

#[test]
fn test_bitmap_arr64() {
    let mut arr = [0xa5a5a5a5a5a5a5a5u64; EXP1_IN_BITS / 64];
    let mut bmap2 = [0u64; EXP1_IN_BITS / 64];

    for nbits in 1..EXP1_IN_BITS {
        bmap2.fill(!0);
        bitmap_to_arr64(&mut arr, &EXP1, nbits);
        bitmap_from_arr64(&mut bmap2, &arr, nbits);
        expect_eq_bitmap(&bmap2, &EXP1, nbits);

        let round_up = nbits.next_multiple_of(BITS_PER_LONG);
        let next_bit = find_next_bit(&bmap2, round_up, nbits);
        assert!(
            next_bit >= round_up,
            "bitmap_copy_arr64(nbits == {nbits}: tail is not safely cleared: {next_bit}"
        );

        if nbits % 64 != 0 {
            let mask = genmask_ull(((nbits - 1) % 64) as u32, 0);
            assert_eq!(
                arr[(nbits - 1) / 64] & !mask,
                0,
                "bitmap_to_arr64(nbits == {nbits}): tail is not safely cleared"
            );
        }

        if nbits < EXP1_IN_BITS - 64 {
            assert_eq!(arr[nbits.div_ceil(64)] as u32, 0xa5a5a5a5);
        }
    }
}

#[test]
fn test_mem_optimisations() {
    let mut bmap1 = [0u64; 16];
    let mut bmap2 = [0u64; 16];

    for start in (0..1024).step_by(8) {
        for nbits in (1..1024 - start).step_by(8) {
            bmap1.fill(0x5a5a5a5a5a5a5a5a);
            bmap2.fill(0x5a5a5a5a5a5a5a5a);

            bitmap_set(&mut bmap1, start, nbits);
            __bitmap_set(&mut bmap2, start, nbits);
            assert!(
                bitmap_equal(&bmap1, &bmap2, 1024),
                "set not equal {start} {nbits}"
            );
            assert!(
                __bitmap_equal(&bmap1, &bmap2, 1024),
                "set not __equal {start} {nbits}"
            );

            bitmap_clear(&mut bmap1, start, nbits);
            __bitmap_clear(&mut bmap2, start, nbits);
            assert!(
                bitmap_equal(&bmap1, &bmap2, 1024),
                "clear not equal {start} {nbits}"
            );
            assert!(
                __bitmap_equal(&bmap1, &bmap2, 1024),
                "clear not __equal {start} {nbits}"
            );
        }
    }
}

const CLUMP_EXP: [u8; 8] = [
    0x01, // 1 bit set
    0x02, // non-edge 1 bit set
    0x00, // zero bits set
    0x38, // 3 bits set across 4-bit boundary
    0x38, // Repeated clump
    0x0F, // 4 bits set
    0xFF, // all bits set
    0x05, // non-adjacent 2 bits set
];

#[test]
fn test_for_each_set_clump8() {
    const CLUMP_EXP_NUMBITS: usize = 64;
    let mut bits = [0u64; 1];

    // Set bitmap to test case.
    bitmap_zero(&mut bits, CLUMP_EXP_NUMBITS);
    bitmap_set(&mut bits, 0, 1); // 0x01
    bitmap_set(&mut bits, 9, 1); // 0x02
    bitmap_set(&mut bits, 27, 3); // 0x28
    bitmap_set(&mut bits, 35, 3); // 0x28
    bitmap_set(&mut bits, 40, 4); // 0x0F
    bitmap_set(&mut bits, 48, 8); // 0xFF
    bitmap_set(&mut bits, 56, 1); // 0x05 - part 1
    bitmap_set(&mut bits, 58, 1); // 0x05 - part 2

    let mut seen = 0;
    for (start, clump) in for_each_set_clump8(&bits, CLUMP_EXP_NUMBITS) {
        // __check_eq_clump8()
        assert!(
            start < CLUMP_EXP_NUMBITS,
            "bit offset for clump out-of-bounds: {start}"
        );
        let exp = CLUMP_EXP[start / 8] as u64;
        assert_ne!(exp, 0, "bit offset for zero clump: got bit offset {start}");
        assert_eq!(clump, exp, "clump at {start}");
        seen += 1;
    }
    assert_eq!(seen, 7);
}

/// Bits 0, 10, ..., 490 and 100-149, as several tests set them.
fn tens_and_range() -> [u64; 8] {
    let mut orig = [0u64; 8];
    for b in (0..500).step_by(10) {
        bitmap_set(&mut orig, b, 1);
    }
    bitmap_set(&mut orig, 100, 50);
    orig
}

/// The complement of [`tens_and_range`] within 500 bits.
fn not_tens_and_range() -> [u64; 8] {
    let mut orig = [0u64; 8];
    bitmap_fill(&mut orig, 500);
    for b in (0..500).step_by(10) {
        bitmap_clear(&mut orig, b, 1);
    }
    bitmap_clear(&mut orig, 100, 50);
    orig
}

#[test]
fn test_for_each_set_bit_wrap() {
    let orig = tens_and_range();
    let mut copy = [0u64; 8];

    for wr in 0..500 {
        bitmap_zero(&mut copy, 500);
        for b in for_each_set_bit_wrap(&orig, 500, wr) {
            bitmap_set(&mut copy, b, 1);
        }
        expect_eq_bitmap(&orig, &copy, 500);
    }
}

#[test]
fn test_for_each_set_bit() {
    let orig = tens_and_range();
    let mut copy = [0u64; 8];

    for b in for_each_set_bit(&orig, 500) {
        bitmap_set(&mut copy, b, 1);
    }
    expect_eq_bitmap(&orig, &copy, 500);
}

#[test]
fn test_for_each_set_bit_from() {
    let orig = tens_and_range();
    let mut copy = [0u64; 8];

    for wr in 0..500 {
        let mut tmp = [0u64; 8];

        bitmap_zero(&mut copy, 500);
        for b in for_each_set_bit_from(wr, &orig, 500) {
            bitmap_set(&mut copy, b, 1);
        }

        bitmap_copy(&mut tmp, &orig, 500);
        bitmap_clear(&mut tmp, 0, wr);
        expect_eq_bitmap(&tmp, &copy, 500);
    }
}

#[test]
fn test_bitmap_weight() {
    let mut b = [0u64; 1];
    let mut b1 = [0u64; 2];

    bitmap_parselist(b"all:1/2", &mut b, 30).unwrap();

    // Test inline implementation.
    let w = bitmap_weight(&b, 30);
    let w1 = bitmap_weight(&b, 15);
    let w2 = bitmap_weight_from(&b, 15, 30);

    assert_eq!(15, w);
    assert_eq!(8, w1);
    assert_eq!(7, w2);

    // Test outline implementation.
    let w = bitmap_weight(&EXP1, EXP1_IN_BITS);
    for bit in 1..EXP1_IN_BITS {
        let w1 = bitmap_weight(&EXP1, bit);
        let w2 = bitmap_weight_from(&EXP1, bit, EXP1_IN_BITS);
        assert_eq!(w1 + w2, w);
    }

    // Test out-of-range.
    let w = bitmap_weight_from(&b, 31, 30);
    assert!(w >= 30);

    // Test bitmap_weight() for correctness in case of some bits set between
    // nbits and end of the last word.
    bitmap_fill(&mut b1, 128);

    // Inline.
    assert_eq!(30, bitmap_weight(&b1, 30));
    assert_eq!(100, bitmap_weight(&b1, 100));

    // Outline.
    for i in 1..128 {
        assert_eq!(i, bitmap_weight(&b1, i));
    }
}

#[test]
fn test_for_each_clear_bit() {
    let orig = not_tens_and_range();
    let mut copy = [0u64; 8];
    bitmap_fill(&mut copy, 500);

    for b in for_each_clear_bit(&orig, 500) {
        bitmap_clear(&mut copy, b, 1);
    }
    expect_eq_bitmap(&orig, &copy, 500);
}

#[test]
fn test_for_each_clear_bit_from() {
    let orig = not_tens_and_range();
    let mut copy = [0u64; 8];

    for wr in 0..500 {
        let mut tmp = [0u64; 8];

        bitmap_fill(&mut copy, 500);
        for b in for_each_clear_bit_from(wr, &orig, 500) {
            bitmap_clear(&mut copy, b, 1);
        }

        bitmap_copy(&mut tmp, &orig, 500);
        bitmap_set(&mut tmp, 0, wr);
        expect_eq_bitmap(&tmp, &copy, 500);
    }
}

#[test]
fn test_for_each_set_bitrange() {
    let orig = tens_and_range();
    let mut copy = [0u64; 8];

    for (s, e) in for_each_set_bitrange(&orig, 500) {
        bitmap_set(&mut copy, s, e - s);
    }
    expect_eq_bitmap(&orig, &copy, 500);
}

#[test]
fn test_for_each_clear_bitrange() {
    let orig = not_tens_and_range();
    let mut copy = [0u64; 8];
    bitmap_fill(&mut copy, 500);

    for (s, e) in for_each_clear_bitrange(&orig, 500) {
        bitmap_clear(&mut copy, s, e - s);
    }
    expect_eq_bitmap(&orig, &copy, 500);
}

#[test]
fn test_for_each_set_bitrange_from() {
    let orig = tens_and_range();
    let mut copy = [0u64; 8];

    for wr in 0..500 {
        let mut tmp = [0u64; 8];

        bitmap_zero(&mut copy, 500);
        for (s, e) in for_each_set_bitrange_from(wr, &orig, 500) {
            bitmap_set(&mut copy, s, e - s);
        }

        bitmap_copy(&mut tmp, &orig, 500);
        bitmap_clear(&mut tmp, 0, wr);
        expect_eq_bitmap(&tmp, &copy, 500);
    }
}

#[test]
fn test_for_each_clear_bitrange_from() {
    let mut orig = [0u64; 8];
    let mut copy = [0u64; 8];

    bitmap_fill(&mut orig, 500);
    // Set individual bits.
    for s in (0..500).step_by(10) {
        bitmap_clear(&mut orig, s, 1);
    }
    // Set range of bits.
    bitmap_set(&mut orig, 100, 50);

    for wr in 0..500 {
        let mut tmp = [0u64; 8];

        bitmap_fill(&mut copy, 500);
        for (s, e) in for_each_clear_bitrange_from(wr, &orig, 500) {
            bitmap_clear(&mut copy, s, e - s);
        }

        bitmap_copy(&mut tmp, &orig, 500);
        bitmap_set(&mut tmp, 0, wr);
        expect_eq_bitmap(&tmp, &copy, 500);
    }
}

/// `struct test_bitmap_cut`: first, cut, nbits, in, expected.
type TestBitmapCut = (usize, usize, usize, [u64; 4], [u64; 4]);

const TEST_CUT: &[TestBitmapCut] = &[
    (0, 0, 8, [0x0000000a, 0, 0, 0], [0x0000000a, 0, 0, 0]),
    (0, 0, 32, [0xdadadead, 0, 0, 0], [0xdadadead, 0, 0, 0]),
    (0, 3, 8, [0x000000aa, 0, 0, 0], [0x00000015, 0, 0, 0]),
    (3, 3, 8, [0x000000aa, 0, 0, 0], [0x00000012, 0, 0, 0]),
    (0, 1, 32, [0xa5a5a5a5, 0, 0, 0], [0x52d2d2d2, 0, 0, 0]),
    (0, 8, 32, [0xdeadc0de, 0, 0, 0], [0x00deadc0, 0, 0, 0]),
    (1, 1, 32, [0x5a5a5a5a, 0, 0, 0], [0x2d2d2d2c, 0, 0, 0]),
    (0, 15, 32, [0xa5a5a5a5, 0, 0, 0], [0x00014b4b, 0, 0, 0]),
    (0, 16, 32, [0xa5a5a5a5, 0, 0, 0], [0x0000a5a5, 0, 0, 0]),
    (15, 15, 32, [0xa5a5a5a5, 0, 0, 0], [0x000125a5, 0, 0, 0]),
    (15, 16, 32, [0xa5a5a5a5, 0, 0, 0], [0x0000a5a5, 0, 0, 0]),
    (16, 15, 32, [0xa5a5a5a5, 0, 0, 0], [0x0001a5a5, 0, 0, 0]),
    (
        BITS_PER_LONG,
        BITS_PER_LONG,
        BITS_PER_LONG,
        [0xa5a5a5a5, 0xa5a5a5a5, 0, 0],
        [0xa5a5a5a5, 0xa5a5a5a5, 0, 0],
    ),
    (
        1,
        BITS_PER_LONG - 1,
        BITS_PER_LONG,
        [0xa5a5a5a5, 0xa5a5a5a5, 0, 0],
        [0x00000001, 0x00000001, 0, 0],
    ),
    (
        0,
        BITS_PER_LONG * 2,
        BITS_PER_LONG * 2 + 1,
        [0xa5a5a5a5, 0x00000001, 0x00000001, 0x00000001],
        [0x00000001, 0, 0, 0],
    ),
    (
        16,
        BITS_PER_LONG * 2 + 1,
        BITS_PER_LONG * 2 + 1 + 16,
        [0x0000ffff, 0x5a5a5a5a, 0x5a5a5a5a, 0x5a5a5a5a],
        [0x2d2dffff, 0, 0, 0],
    ),
];

#[test]
fn test_bitmap_cut() {
    // C cuts from in = &b[1] to out = &b[0], a partial overlap. Rust cannot
    // alias them, so this does the memmove() and then cuts in place.
    for &(first, cut, nbits, input, expected) in TEST_CUT {
        let mut b = [0u64; 5];
        b[1..5].copy_from_slice(&input);

        let len = nbits.div_ceil(BITS_PER_LONG);
        b.copy_within(1..1 + len, 0);
        bitmap_cut_in_place(&mut b, first, cut, nbits);

        expect_eq_bitmap(&expected, &b, nbits);

        // And with separate buffers.
        let mut out = [0u64; 5];
        bitmap_cut(&mut out, &input, first, cut, nbits);
        expect_eq_bitmap(&expected, &out, nbits);
    }
}

const SMALL_BITMAP: [u64; 1] = [0x3333333311111111];
const SMALL_MASK: &str = "33333333,11111111\n";
const SMALL_LIST: &str = "0,4,8,12,16,20,24,28,32-33,36-37,40-41,44-45,48-49,52-53,56-57,60-61\n";
const LARGE_BITMAP: [u64; 40] = [0x3333333311111111; 40];

#[test]
fn test_bitmap_print_buf() {
    let test_print: [(&[u64], usize, &str, &str); 2] = [
        (
            &SMALL_BITMAP,
            SMALL_BITMAP.len() * 64,
            SMALL_MASK,
            SMALL_LIST,
        ),
        (
            &LARGE_BITMAP,
            LARGE_BITMAP.len() * 64,
            LARGE_MASK,
            LARGE_LIST,
        ),
    ];
    let mut print_buf = vec![0u8; PAGE_SIZE * 2];

    for (bitmap, nbits, mask, list) in test_print {
        // The copy includes the NUL.
        let with_nul = |s: &str| [s.as_bytes(), b"\0"].concat();

        let n = bitmap_print_bitmask_to_buf(&mut print_buf, bitmap, nbits, 0);
        assert_eq!(mask.len() + 1, n);
        assert_eq!(&print_buf[..n], &with_nul(mask)[..]);

        let n = bitmap_print_list_to_buf(&mut print_buf, bitmap, nbits, 0);
        assert_eq!(list.len() + 1, n);
        assert_eq!(&print_buf[..n], &with_nul(list)[..]);

        // Test by non-zero offset.
        if list.len() > PAGE_SIZE {
            let n = bitmap_print_list_to_buf(&mut print_buf[..PAGE_SIZE], bitmap, nbits, PAGE_SIZE);
            assert_eq!(list.len() + 1 - PAGE_SIZE, n);
            assert_eq!(&print_buf[..n], &with_nul(list)[PAGE_SIZE..]);
        }
    }
}

/// C checks that these fold to compile-time constants; here they are
/// checked at run time.
#[test]
fn test_bitmap_const_eval() {
    let mut bitmap = [0u64; 1];
    let initvar = bit(2);
    let mut bitopvar = [0u64; 1];
    let mut var = [0u64; 1];

    // Equals to `unsigned long bitmap[1] = { GENMASK(6, 5), }`.
    bitmap_clear(&mut bitmap, 0, BITS_PER_LONG);
    if !test_bit(7, &bitmap) {
        bitmap_set(&mut bitmap, 5, 2);
    }

    // Equals to `unsigned long bitopvar = BIT(20)`.
    change_bit(31, &mut bitopvar);
    let src = bitopvar;
    bitmap_shift_right(&mut bitopvar, &src, 11, BITS_PER_LONG);

    // Equals to `unsigned long var = BIT(25)`.
    var[0] |= bit(25);
    if var[0] & bit(0) != 0 {
        var[0] ^= genmask(9, 6);
    }

    // __const_hweight<32|64>(GENMASK(6, 5)) == 2
    assert_eq!(bitmap_weight(&bitmap, 20), 2);

    // !(BIT(31) & BIT(18)) == 1
    assert!(!test_bit(18, &bitopvar));

    // BIT(2) & GENMASK(14, 8) == 0
    assert_eq!(initvar & genmask(14, 8), 0);

    // ~BIT(25)
    assert_eq!(!var[0], !bit(25));

    // ~BIT(25) | BIT(25) == ~0UL
    let src = var;
    bitmap_complement(&mut var, &src, BITS_PER_LONG);
    assign_bit(25, &mut var, true);

    // !(~(~0UL)) == 1
    assert!(bitmap_full(&var, BITS_PER_LONG));
}

/// Test bitmap should be big enough to include the cases when start is not
/// in the first word, and start+nbits lands in the following word.
const TEST_BIT_LEN: usize = 1000;
const TEST_BIT_WORDS: usize = TEST_BIT_LEN.div_ceil(64);

/// Helper function to test bitmap_write() overwriting the chosen byte
/// pattern.
fn test_bitmap_write_helper(pattern: &str) {
    let mut bitmap = [0u64; TEST_BIT_WORDS];
    let mut exp_bitmap = [0u64; TEST_BIT_WORDS];
    let mut pat_bitmap = [0u64; TEST_BIT_WORDS];

    // Only parse the pattern once and store the result in the intermediate
    // bitmap.
    bitmap_parselist(pattern.as_bytes(), &mut pat_bitmap, TEST_BIT_LEN).unwrap();

    // Check that writing a single bit does not accidentally touch the
    // adjacent bits.
    for i in 0..TEST_BIT_LEN {
        bitmap_copy(&mut bitmap, &pat_bitmap, TEST_BIT_LEN);
        bitmap_copy(&mut exp_bitmap, &pat_bitmap, TEST_BIT_LEN);
        for b in 0..=1u64 {
            bitmap_write(&mut bitmap, b, i, 1);
            assign_bit(i, &mut exp_bitmap, b != 0);
            expect_eq_bitmap(&exp_bitmap, &bitmap, TEST_BIT_LEN);
        }
    }

    // Ensure writing 0 bits does not change anything.
    bitmap_copy(&mut bitmap, &pat_bitmap, TEST_BIT_LEN);
    bitmap_copy(&mut exp_bitmap, &pat_bitmap, TEST_BIT_LEN);
    for i in 0..TEST_BIT_LEN {
        bitmap_write(&mut bitmap, !0, i, 0);
        expect_eq_bitmap(&exp_bitmap, &bitmap, TEST_BIT_LEN);
    }

    for nbits in (1..=BITS_PER_LONG).rev() {
        let w = 0xdeadbeefdeadbeefu64 >> (BITS_PER_LONG - nbits);
        for i in 0..=TEST_BIT_LEN - nbits {
            bitmap_copy(&mut bitmap, &pat_bitmap, TEST_BIT_LEN);
            bitmap_copy(&mut exp_bitmap, &pat_bitmap, TEST_BIT_LEN);
            for n in 0..nbits {
                assign_bit(i + n, &mut exp_bitmap, w & bit(n as u32) != 0);
            }
            bitmap_write(&mut bitmap, w, i, nbits);
            expect_eq_bitmap(&exp_bitmap, &bitmap, TEST_BIT_LEN);
            let r = bitmap_read(&bitmap, i, nbits);
            assert_eq!(r, w);
        }
    }
}

#[test]
fn test_bitmap_read_write() {
    let pattern = ["", "all:1/2", "all"];
    let mut bitmap = [0u64; TEST_BIT_WORDS];

    // Reading/writing zero bits should not crash the kernel.
    bitmap_write(&mut [], 0, 0, 0);
    // Return value of bitmap_read() is undefined here.
    bitmap_read(&[], 0, 0);

    // Reading/writing more than BITS_PER_LONG bits should not crash the
    // kernel.
    bitmap_write(&mut [], 0, 0, BITS_PER_LONG + 1);
    // Return value of bitmap_read() is undefined here.
    bitmap_read(&[], 0, BITS_PER_LONG + 1);

    // Ensure that bitmap_read() reads the same value that was previously
    // written, and two consequent values are correctly merged. The
    // resulting bit pattern is asymmetric to rule out possible issues with
    // bit numeration order.
    for i in 0..TEST_BIT_LEN - 7 {
        bitmap_zero(&mut bitmap, TEST_BIT_LEN);

        bitmap_write(&mut bitmap, 0b10101, i, 5);
        assert_eq!(0b10101, bitmap_read(&bitmap, i, 5));

        bitmap_write(&mut bitmap, 0b101, i + 5, 3);
        assert_eq!(0b101, bitmap_read(&bitmap, i + 5, 3));

        assert_eq!(0b10110101, bitmap_read(&bitmap, i, 8));
    }

    for p in pattern {
        test_bitmap_write_helper(p);
    }
}

/// nbits == 0 is most commonly not a valid case. Bitmap users should
/// revisit the caller logic. Bitmap API doesn't provide any guarantees on
/// returned value. The pointers are not dereferenced; here the slices are
/// empty. The return value is intentionally ignored.
#[test]
fn test_zero_nbits() {
    let e: &[u64] = &[];
    let mut m = [0u64; 0];

    bitmap_clear(&mut m, 0, 0);
    bitmap_complement(&mut m, e, 0);
    bitmap_copy(&mut m, e, 0);
    bitmap_copy_clear_tail(&mut m, e, 0);
    bitmap_fill(&mut m, 0);
    bitmap_from_arr32(&mut m, &[], 0);
    bitmap_from_arr64(&mut m, e, 0);
    bitmap_or(&mut m, e, e, 0);
    bitmap_set(&mut m, 0, 0);
    bitmap_shift_left(&mut m, e, 0, 0);
    bitmap_shift_right(&mut m, e, 0, 0);
    bitmap_to_arr32(&mut [], e, 0);
    bitmap_to_arr64(&mut m, e, 0);
    bitmap_write(&mut m, 0, 0, 0);
    bitmap_xor(&mut m, e, e, 0);
    bitmap_zero(&mut m, 0);

    let _ = bitmap_and(&mut m, e, e, 0);
    let _ = bitmap_empty(e, 0);
    let _ = bitmap_equal(e, e, 0);
    let _ = bitmap_full(e, 0);
    let _ = bitmap_or_equal(e, e, e, 0);
    let _ = bitmap_read(e, 0, 0);
    let _ = bitmap_subset(e, e, 0);
    let _ = bitmap_weight(e, 0);
    let _ = bitmap_weight_and(e, e, 0);
    let _ = bitmap_weight_andnot(e, e, 0);
    let _ = bitmap_weight_from(e, 0, 0);
    let _ = bitmap_weighted_or(&mut m, e, e, 0);

    let _ = find_first_and_and_bit(e, e, e, 0);
    let _ = find_first_and_bit(e, e, 0);
    let _ = find_first_andnot_bit(e, e, 0);
    let _ = find_first_bit(e, 0);
    let _ = find_first_zero_bit(e, 0);
    let _ = find_last_bit(e, 0);
    let _ = find_next_and_bit(e, e, 0, 0);
    let _ = find_next_andnot_bit(e, e, 0, 0);
    let _ = find_next_bit(e, 0, 0);
    let _ = find_next_clump8(&mut 0, e, 0, 0);
    let _ = find_next_zero_bit(e, 0, 0);
    let _ = find_nth_and_bit(e, e, 0, 0);
    let _ = find_nth_bit(e, 0, 0);
}

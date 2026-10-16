// SPDX-License-Identifier: GPL-2.0-or-later
//! Host tests for `lib/bitmap-str.c`'s list parser. The examples come from
//! the C comments; Linux's `test_bitmap.c` exercises this parser through the
//! kernel, and these tests run the same forms on the host.

use rinux_lib::bitmap::bitmap_size;
use rinux_lib::bitmap_str::bitmap_parselist;
use rinux_lib::errno::{EINVAL, EOVERFLOW, ERANGE};

/// Parse `s` into a mask of `nbits` bits and return the set bits.
fn parse(s: &[u8], nbits: usize) -> Result<Vec<usize>, rinux_lib::errno::Errno> {
    let mut mask = vec![0u64; bitmap_size(nbits)];
    bitmap_parselist(s, &mut mask, nbits)?;
    Ok((0..nbits)
        .filter(|&b| mask[b / 64] >> (b % 64) & 1 == 1)
        .collect())
}

#[test]
fn single_bits_and_ranges() {
    assert_eq!(parse(b"1,3-5", 8), Ok(vec![1, 3, 4, 5]));
    assert_eq!(parse(b"5", 8), Ok(vec![5]));
    assert_eq!(parse(b"0-7", 8), Ok((0..8).collect::<Vec<_>>()));
}

#[test]
fn separators_and_white_space_are_skipped() {
    assert_eq!(parse(b" 1 ,, 2\n", 8), Ok(vec![1, 2]));
    assert_eq!(parse(b",1,\t2\r\n", 8), Ok(vec![1, 2]));
}

#[test]
fn empty_list_sets_nothing() {
    assert_eq!(parse(b"", 8), Ok(vec![]));
    assert_eq!(parse(b"\n", 8), Ok(vec![]));
}

#[test]
fn all_sets_every_bit() {
    assert_eq!(parse(b"all", 5), Ok((0..5).collect::<Vec<_>>()));
    assert_eq!(parse(b"ALL", 3), Ok(vec![0, 1, 2]));
}

#[test]
fn n_is_the_last_bit() {
    assert_eq!(parse(b"N", 8), Ok(vec![7]));
    assert_eq!(parse(b"2-N", 8), Ok(vec![2, 3, 4, 5, 6, 7]));
}

#[test]
fn strided_pattern_from_the_c_comment() {
    // 0-1023:2/256 ==> 0,1,256,257,512,513,768,769
    assert_eq!(
        parse(b"0-1023:2/256", 1024),
        Ok(vec![0, 1, 256, 257, 512, 513, 768, 769])
    );
}

#[test]
fn a_pattern_without_a_range_uses_the_whole_span() {
    // 3:... with a range end of 3 means group and used are both 4.
    assert_eq!(parse(b"2-3:2/4", 8), Ok(vec![2, 3]));
    assert_eq!(parse(b"0-7:2/3", 8), Ok(vec![0, 1, 3, 4, 6, 7]));
}

#[test]
fn bits_are_cleared_before_parsing() {
    let mut mask = vec![u64::MAX];
    bitmap_parselist(b"1", &mut mask, 8).unwrap();
    assert_eq!(mask[0], 0b10);
}

#[test]
fn malformed_regions_are_einval() {
    assert_eq!(parse(b"5-2", 8), Err(EINVAL), "range runs backwards");
    assert_eq!(parse(b"1-", 8), Err(EINVAL), "missing range end");
    assert_eq!(parse(b"abc", 8), Err(EINVAL));
    assert_eq!(parse(b"1:2/3", 8), Err(EINVAL), "a pattern needs a range");
    assert_eq!(parse(b"1-3:x/2", 8), Err(EINVAL));
    assert_eq!(parse(b"1-3:4/2", 8), Err(EINVAL), "used exceeds group");
    assert_eq!(parse(b"1-3:2/0", 8), Err(EINVAL), "zero group");
    assert_eq!(parse(b"1-3:2", 8), Err(EINVAL), "missing group");
}

#[test]
fn a_bit_past_the_mask_is_erange() {
    assert_eq!(parse(b"8", 8), Err(ERANGE));
    assert_eq!(parse(b"0-8", 8), Err(ERANGE));
}

#[test]
fn numbers_wider_than_32_bits_are_eoverflow() {
    assert_eq!(parse(b"4294967296", 8), Err(EOVERFLOW));
    assert_eq!(parse(b"99999999999999999999999", 8), Err(EOVERFLOW));
}

mod hex {
    use rinux_lib::bitmap::bitmap_size;
    use rinux_lib::bitmap_str::bitmap_parse;
    use rinux_lib::errno::{EINVAL, EOVERFLOW};

    /// The first word of the mask: every case here fits in one 64-bit word.
    fn parse(s: &[u8], nbits: usize) -> Result<Vec<u64>, rinux_lib::errno::Errno> {
        let mut mask = vec![0u64; bitmap_size(nbits)];
        bitmap_parse(s, usize::MAX, &mut mask, nbits)?;
        Ok(vec![mask[0]])
    }

    #[test]
    fn a_short_string_fills_the_low_bits() {
        assert_eq!(parse(b"ff", 8), Ok(vec![0xff]));
        assert_eq!(parse(b"ff\n", 8), Ok(vec![0xff]));
    }

    #[test]
    fn groups_split_on_commas_and_read_from_the_right() {
        // "1,,5" is chunk 0 = 5 and chunk 1 = 1.
        assert_eq!(parse(b"1,,5", 64), Ok(vec![0x0000_0001_0000_0005]));
    }

    #[test]
    fn a_partial_last_group_is_cleared_above_nbits() {
        // Two groups for 40 bits: 0xffffffff then 0xff; bits 40..63 stay clear.
        assert_eq!(parse(b"ff,ffffffff", 40), Ok(vec![0x0000_00ff_ffff_ffff]));
    }

    #[test]
    fn bits_past_nbits_are_eoverflow() {
        assert_eq!(parse(b"1ff", 8), Err(EOVERFLOW));
        assert_eq!(
            parse(b"123456789", 32),
            Err(EOVERFLOW),
            "more than 32 bits in a group"
        );
    }

    #[test]
    fn a_non_hex_character_is_einval() {
        assert_eq!(parse(b"x", 8), Err(EINVAL));
        assert_eq!(parse(b"1g", 8), Err(EINVAL));
    }

    #[test]
    fn an_empty_string_clears_the_mask() {
        let mut mask = vec![u64::MAX];
        bitmap_parse(b"", usize::MAX, &mut mask, 8).unwrap();
        assert_eq!(mask[0] & 0xff, 0);
    }

    #[test]
    fn buflen_limits_how_much_is_read() {
        let mut mask = vec![0u64; 1];
        bitmap_parse(b"ffff", 2, &mut mask, 8).unwrap();
        assert_eq!(mask[0] & 0xff, 0xff);
    }
}

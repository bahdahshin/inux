// SPDX-License-Identifier: GPL-2.0-only
//! Host port of `lib/test_bitops.c`.
//!
//! The C module sets five bits in a 256-bit map, checks `get_count_order()`
//! and `get_count_order_long()` against the tables, clears the bits, and
//! checks that no bit is left set. The `fns()` timing loop is a benchmark
//! and is not ported.

use rinux_lib::bitops::{clear_bit, get_count_order, get_count_order_long, set_bit};
use rinux_lib::find_bit::find_first_bit;

/// `enum bitops_fun`.
const BITOPS_4: usize = 4;
const BITOPS_7: usize = 7;
const BITOPS_11: usize = 11;
const BITOPS_31: usize = 31;
const BITOPS_88: usize = 88;
const BITOPS_LAST: usize = 255;
const BITOPS_LENGTH: usize = 256;

const ORDER_COMB: [(u32, i32); 7] = [
    (0x0000_0003, 2),
    (0x0000_0004, 2),
    (0x0000_1fff, 13),
    (0x0000_2000, 13),
    (0x5000_0000, 31),
    (0x8000_0000, 31),
    (0x8000_3000, 32),
];

const ORDER_COMB_LONG: [(u64, i32); 7] = [
    (0x0000_0003_0000_0000, 34),
    (0x0000_0004_0000_0000, 34),
    (0x0000_1fff_0000_0000, 45),
    (0x0000_2000_0000_0000, 45),
    (0x5000_0000_0000_0000, 63),
    (0x8000_0000_0000_0000, 63),
    (0x8000_3000_0000_0000, 64),
];

#[test]
fn count_order_matches_the_tables() {
    for (word, order) in ORDER_COMB {
        assert_eq!(get_count_order(word), order, "get_count_order({word:#x})");
    }
    for (word, order) in ORDER_COMB {
        assert_eq!(
            get_count_order_long(word as u64),
            order,
            "get_count_order_long({word:#x})"
        );
    }
    for (word, order) in ORDER_COMB_LONG {
        assert_eq!(
            get_count_order_long(word),
            order,
            "get_count_order_long({word:#x})"
        );
    }
}

#[test]
fn set_then_clear_leaves_no_bit_set() {
    let mut g_bitmap = [0u64; BITOPS_LENGTH / 64];
    for bit in [BITOPS_4, BITOPS_7, BITOPS_11, BITOPS_31, BITOPS_88] {
        set_bit(bit, &mut g_bitmap);
    }
    assert_eq!(find_first_bit(&g_bitmap, BITOPS_LAST), BITOPS_4);
    for bit in [BITOPS_4, BITOPS_7, BITOPS_11, BITOPS_31, BITOPS_88] {
        clear_bit(bit, &mut g_bitmap);
    }
    assert_eq!(find_first_bit(&g_bitmap, BITOPS_LAST), BITOPS_LAST);
}

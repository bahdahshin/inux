// SPDX-License-Identifier: GPL-2.0-only
//! The bit-length table for bytes, used by the generic count-leading-zeros
//! code.
//!
//! Ported from `lib/clz_tab.c`. The C file is a 256-entry literal table
//! whose entry `i` is the number of bits needed to write `i`, that is
//! `fls(i)`. Here the table is built at compile time from that rule, so the
//! data cannot drift from the definition.

/// `__clz_tab`: entry `i` is the bit length of `i`; entry `0` is `0`.
pub const CLZ_TAB: [u8; 256] = build();

const fn build() -> [u8; 256] {
    let mut table = [0u8; 256];
    let mut i = 0;
    while i < 256 {
        table[i] = 8 - (i as u8).leading_zeros() as u8;
        i += 1;
    }
    table
}

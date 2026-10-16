// SPDX-License-Identifier: LGPL-2.1-or-later
//! Fixed-point logarithms.
//!
//! Ported from `lib/math/int_log.c`.
//!
//! Copyright (C) 2006 Christoph Pfister (christophpfister@gmail.com)

include!("int_log_table.rs");

/// `intlog2()`: `log2(value) * 2^24`.
///
/// `log2(0)` is undefined; it warns and returns 0.
pub fn intlog2(value: u32) -> u32 {
    if crate::warn_on!(value == 0) {
        return 0;
    }

    // Find the most significant bit (counting from 0).
    let msb = 31 - value.leading_zeros();

    // log2(2^x * y) * 2^24 = x * 2^24 + log2(y) * 2^24, with x = msb and
    // 1 <= y < 2. Shift so the msb is bit 31; the top nine bits index the
    // table, dropping the always-set top bit.
    let significand = value << (31 - msb);
    let logentry = (significand >> 23) as usize % LOGTABLE.len();

    // Interpolate between this table entry and the next using the bits that
    // were not used for the lookup. The mask corrects for the wrap at 256.
    let next = LOGTABLE[(logentry + 1) % LOGTABLE.len()] as u32;
    let step = next.wrapping_sub(LOGTABLE[logentry] as u32) & 0xffff;
    let interpolation = (significand & 0x7f_ffff).wrapping_mul(step) >> 15;

    (msb << 24) + ((LOGTABLE[logentry] as u32) << 8) + interpolation
}

/// `intlog10()`: `log10(value) * 2^24`, computed as `log2(value) * log10(2)`.
///
/// `log10(0)` is undefined; it warns and returns 0.
pub fn intlog10(value: u32) -> u32 {
    if crate::warn_on!(value == 0) {
        return 0;
    }
    let log = intlog2(value) as u64;
    ((log * 646_456_993) >> 31) as u32
}

// SPDX-License-Identifier: GPL-2.0+
//! Clock source helpers.
//!
//! A partial port of `kernel/time/clocksource.c`: `clocks_calc_mult_shift()`.
//! The clocksource registry and watchdog come with timekeeping.

/// `clocks_calc_mult_shift()`: finds `(mult, shift)` so that
/// `(x * mult) >> shift` converts from a clock of frequency `from` to one
/// of frequency `to`, as accurately as possible while values up to
/// `maxsec` seconds' worth of `from` ticks don't overflow 64 bits.
pub fn clocks_calc_mult_shift(from: u32, to: u32, maxsec: u32) -> (u32, u32) {
    // Calculate the shift factor which is limiting the conversion range.
    let mut tmp = (maxsec as u64 * from as u64) >> 32;
    let mut sftacc: u32 = 32;
    while tmp != 0 {
        tmp >>= 1;
        sftacc -= 1;
    }

    // Find the conversion shift/mult pair which has the best accuracy and
    // fits the maxsec conversion range.
    let mut sft: u32 = 32;
    while sft > 0 {
        tmp = (to as u64) << sft;
        tmp += from as u64 / 2;
        tmp /= from as u64;
        if tmp >> sftacc == 0 {
            break;
        }
        sft -= 1;
    }
    (tmp as u32, sft)
}

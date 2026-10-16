// SPDX-License-Identifier: GPL-2.0
//! The Internet checksum (RFC 1071).
//!
//! A small port of the generic C `csum_partial()`/`ip_compute_csum()` from
//! `lib/checksum.c`: sum the data as big-endian 16-bit words, fold the
//! carries back in, and take the one's complement. rinux uses the portable
//! version rather than the per-arch assembly in `arch/x86/lib/csum-*`.

/// Sums `data` as 16-bit big-endian words into a 32-bit accumulator, with a
/// starting value (so checksums can be chained).
pub fn csum_partial(data: &[u8], start: u32) -> u32 {
    let mut sum = start;
    let mut i = 0;
    while i + 1 < data.len() {
        sum = sum.wrapping_add(u16::from_be_bytes([data[i], data[i + 1]]) as u32);
        i += 2;
    }
    if i < data.len() {
        // Odd trailing byte: it is the high byte of the final word.
        sum = sum.wrapping_add((data[i] as u32) << 8);
    }
    sum
}

/// Folds a 32-bit accumulator to 16 bits and takes the one's complement,
/// giving the value to store in a checksum field.
pub fn csum_fold(mut sum: u32) -> u16 {
    while sum >> 16 != 0 {
        sum = (sum & 0xffff) + (sum >> 16);
    }
    !(sum as u16)
}

/// `ip_compute_csum()`: the Internet checksum of a single buffer.
pub fn ip_compute_csum(data: &[u8]) -> u16 {
    csum_fold(csum_partial(data, 0))
}

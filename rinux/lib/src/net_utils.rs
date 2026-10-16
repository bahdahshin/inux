// SPDX-License-Identifier: GPL-2.0
//! Parse a MAC address written as six colon-separated hex pairs.
//!
//! Ported from `lib/net_utils.c`. The C function writes into a caller's
//! six-byte buffer only after the whole string has been checked. Here the
//! result is returned, so a bad string never produces a partial address.

use crate::ctype::isxdigit;
use crate::hexdump::hex_to_bin;

/// `ETH_ALEN`: bytes in a MAC address.
pub const ETH_ALEN: usize = 6;
/// `MAC_ADDR_STR_LEN`: `"aa:bb:cc:dd:ee:ff"` without a NUL.
pub const MAC_ADDR_STR_LEN: usize = 3 * ETH_ALEN - 1;

/// `mac_pton()`: parse `"xx:xx:xx:xx:xx:xx"`, with either case of hex digit.
///
/// Returns `None` when the string is shorter than 17 bytes before a NUL,
/// when a digit is not hex, or when a separator is not `:`. Bytes after a
/// NUL inside the first 17 are not read.
pub fn mac_pton(s: &[u8]) -> Option<[u8; ETH_ALEN]> {
    let window = &s[..s.len().min(MAC_ADDR_STR_LEN)];
    if window.len() < MAC_ADDR_STR_LEN || window.contains(&0) {
        return None;
    }
    for i in 0..ETH_ALEN {
        if !isxdigit(s[i * 3]) || !isxdigit(s[i * 3 + 1]) {
            return None;
        }
        if i != ETH_ALEN - 1 && s[i * 3 + 2] != b':' {
            return None;
        }
    }
    let mut mac = [0u8; ETH_ALEN];
    for (i, byte) in mac.iter_mut().enumerate() {
        let hi = hex_to_bin(s[i * 3]) as u8;
        let lo = hex_to_bin(s[i * 3 + 1]) as u8;
        *byte = (hi << 4) | lo;
    }
    Some(mac)
}

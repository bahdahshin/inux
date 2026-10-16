// SPDX-License-Identifier: GPL-2.0
//! Count the set bits in a byte region.
//!
//! Ported from `lib/memweight.c`.

/// `memweight()`: the number of 1-bits in `bytes`.
///
/// The C version reads whole `unsigned long`s with `bitmap_weight()` after a
/// byte-wise head and before a byte-wise tail. The word-at-a-time path is a
/// performance detail, so this port counts each byte; the result is the same
/// on any byte order.
pub fn memweight(bytes: &[u8]) -> usize {
    bytes.iter().map(|b| b.count_ones() as usize).sum()
}

#[cfg(test)]
mod tests {
    use super::memweight;

    #[test]
    fn empty_and_small_regions() {
        assert_eq!(memweight(&[]), 0);
        assert_eq!(memweight(&[0x00]), 0);
        assert_eq!(memweight(&[0xff]), 8);
        assert_eq!(memweight(&[0x01, 0x80, 0x0f]), 6);
    }

    #[test]
    fn long_regions_match_a_bit_loop() {
        let mut seed: u32 = 1;
        let mut data = [0u8; 1000];
        for b in data.iter_mut() {
            seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12_345);
            *b = (seed >> 16) as u8;
        }
        for len in [0, 1, 7, 8, 9, 63, 64, 65, 1000] {
            let slow: usize = data[..len]
                .iter()
                .map(|&b| (0..8).filter(|i| b >> i & 1 == 1).count())
                .sum();
            assert_eq!(memweight(&data[..len]), slow, "len {len}");
        }
    }
}

// SPDX-License-Identifier: GPL-2.0
//! Host tests for the random draws the lib helpers take (`find_random_bit()`
//! in `lib/find_bit.c` and the `uuid` generators in `lib/uuid.c`), over a
//! fixed sequence so every draw is known.

use rinux_lib::find_bit::find_random_bit;
use rinux_lib::random::RandomSource;
use rinux_lib::uuid::{generate_random_guid_from, generate_random_uuid_from};

struct Seq<'a>(&'a [u32]);

impl RandomSource for Seq<'_> {
    fn get_random_u32(&mut self) -> u32 {
        let (&first, rest) = self.0.split_first().expect("ran out of draws");
        self.0 = rest;
        first
    }
}

#[test]
fn find_random_bit_with_no_bits_gives_size() {
    let mut rng = Seq(&[]);
    assert_eq!(find_random_bit(&[0], 64, &mut rng), 64);
}

#[test]
fn find_random_bit_with_one_bit_takes_no_draw() {
    let mut rng = Seq(&[]);
    assert_eq!(find_random_bit(&[1 << 9], 64, &mut rng), 9);
}

#[test]
fn find_random_bit_picks_the_nth_set_bit() {
    // Bits 3, 10 and 40 are set. Three choices: draw 0 is rejected under the
    // threshold, so the next draw 0x8000_0000 gives index 1, which is bit 10.
    let bits = [(1u64 << 3) | (1 << 10) | (1 << 40)];
    let mut rng = Seq(&[0, 0x8000_0000]);
    assert_eq!(find_random_bit(&bits, 64, &mut rng), 10);
}

#[test]
fn random_uuid_sets_version_four_and_the_variant() {
    let mut uuid = [0u8; 16];
    generate_random_uuid_from(&mut uuid, &mut Seq(&[0xffff_ffff; 4]));
    assert_eq!(uuid[6] & 0xf0, 0x40);
    assert_eq!(uuid[8] & 0xc0, 0x80);
}

#[test]
fn random_guid_sets_version_four_and_the_variant() {
    let mut guid = [0u8; 16];
    generate_random_guid_from(&mut guid, &mut Seq(&[0; 4]));
    // A GUID's version is in byte 7, unlike a UUID's byte 6.
    assert_eq!(guid[7] & 0xf0, 0x40);
    assert_eq!(guid[8] & 0xc0, 0x80);
}

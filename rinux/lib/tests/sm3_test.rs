// SPDX-License-Identifier: GPL-2.0-or-later
//! Host tests for SM3, against the GB/T 32905 examples and split updates.

use rinux_lib::crypto::sm3::{Sm3Ctx, sm3};

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[test]
fn abc_matches_the_standard_example() {
    assert_eq!(
        hex(&sm3(b"abc")),
        "66c7f0f462eeedd9d1f2d46bdc10e4e24167c4875cf2f7a2297da02b8f4ba8e0"
    );
}

#[test]
fn sixty_four_byte_message_matches_the_standard_example() {
    let msg = b"abcd".repeat(16);
    assert_eq!(
        hex(&sm3(&msg)),
        "debe9ff92275b8a138604889c18e5a4d6fdb70e5387e5765293dcba39c0c5732"
    );
}

#[test]
fn split_updates_match_one_update() {
    let msg: Vec<u8> = (0..300).map(|i| (i * 17 + 3) as u8).collect();
    let whole = sm3(&msg);
    for cut in [1, 55, 56, 63, 64, 65, 120, 128, 200] {
        let mut ctx = Sm3Ctx::new();
        ctx.update(&msg[..cut]);
        ctx.update(&msg[cut..]);
        assert_eq!(ctx.finalize(), whole, "cut {cut}");
    }
}

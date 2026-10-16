// SPDX-License-Identifier: GPL-2.0-or-later
//! Host tests for unkeyed BLAKE2s-256, against the RFC 7693 vectors and a
//! split-update check. Linux has no KUnit file for the generic code.

use rinux_lib::crypto::blake2s::Blake2sCtx;

fn digest(parts: &[&[u8]]) -> String {
    let mut ctx = Blake2sCtx::new(32);
    for p in parts {
        ctx.update(p);
    }
    let mut out = [0u8; 32];
    ctx.finalize(&mut out);
    out.iter().map(|b| format!("{b:02x}")).collect()
}

#[test]
fn empty_message_matches_the_rfc_vector() {
    assert_eq!(
        digest(&[]),
        "69217a3079908094e11121d042354a7c1f55b6482ca1a51e1b250dfd1ed0eef9"
    );
}

#[test]
fn abc_matches_the_rfc_vector() {
    assert_eq!(
        digest(&[b"abc"]),
        "508c5e8c327c14e2e1a72ba34eeb452f37458b209ed63a294d999b4c86675982"
    );
}

#[test]
fn split_updates_match_one_update_across_block_edges() {
    let msg: Vec<u8> = (0..300).map(|i| (i * 31 + 7) as u8).collect();
    let whole = digest(&[&msg]);
    for cut in [1, 63, 64, 65, 127, 128, 129, 200] {
        assert_eq!(digest(&[&msg[..cut], &msg[cut..]]), whole, "cut {cut}");
    }
    let bytes: Vec<&[u8]> = msg.chunks(7).collect();
    assert_eq!(digest(&bytes), whole);
}

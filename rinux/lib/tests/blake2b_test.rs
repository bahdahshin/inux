// SPDX-License-Identifier: GPL-2.0-or-later
//! Host tests for unkeyed BLAKE2b-512, against the RFC 7693 vectors and a
//! split-update check across block edges.

use rinux_lib::crypto::blake2b::Blake2bCtx;

fn digest(parts: &[&[u8]]) -> String {
    let mut ctx = Blake2bCtx::new(64);
    for p in parts {
        ctx.update(p);
    }
    let mut out = [0u8; 64];
    ctx.finalize(&mut out);
    out.iter().map(|b| format!("{b:02x}")).collect()
}

#[test]
fn empty_message_matches_the_rfc_vector() {
    assert_eq!(
        digest(&[]),
        "786a02f742015903c6c6fd852552d272912f4740e15847618a86e217f71f5419d25e1031afee585313896444934eb04b903a685b1448b755d56f701afe9be2ce"
    );
}

#[test]
fn abc_matches_the_rfc_vector() {
    assert_eq!(
        digest(&[b"abc"]),
        "ba80a53f981c4d0d6a2797b69f12f6e94c212f14685ac4b74b12bb6fdbffa2d17d87c5392aab792dc252d5de4533cc9518d38aa8dbf1925ab92386edd4009923"
    );
}

#[test]
fn split_updates_match_one_update_across_block_edges() {
    let msg: Vec<u8> = (0..400).map(|i| (i * 31 + 7) as u8).collect();
    let whole = digest(&[&msg]);
    for cut in [1, 127, 128, 129, 255, 256, 257, 300] {
        assert_eq!(digest(&[&msg[..cut], &msg[cut..]]), whole, "cut {cut}");
    }
}

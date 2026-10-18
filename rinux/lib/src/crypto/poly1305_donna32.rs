// SPDX-License-Identifier: GPL-2.0 OR MIT
//! Poly1305 with 26-bit limbs, the 32-bit core.
//!
//! Ported from `lib/crypto/poly1305-donna32.c`: `poly1305_core_setkey()`,
//! `poly1305_core_blocks()` and `poly1305_core_emit()`. The field is five
//! 26-bit limbs and each product is a 64-bit integer, as in the 32-bit C code.
//! It computes the same tag as the 44-bit core in [`super::poly1305`], which is
//! what the tests check. The one-shot [`poly1305_tag`] is the update and final
//! steps of `lib/crypto/poly1305.c` for this core.

/// The clamped key `r` as five 26-bit limbs, and `5 * r[1..4]` for the reduction.
#[derive(Clone, Copy, Debug)]
pub struct Poly1305Key32 {
    r: [u32; 5],
    s: [u32; 4],
}

/// The accumulator `h`, five 26-bit limbs.
#[derive(Clone, Copy, Debug, Default)]
pub struct Poly1305State32 {
    h: [u32; 5],
}

fn le32(b: &[u8]) -> u32 {
    u32::from_le_bytes([b[0], b[1], b[2], b[3]])
}

/// `poly1305_core_setkey()`: clamp `r` from the first 16 key bytes.
pub fn poly1305_core_setkey(raw: &[u8; 16]) -> Poly1305Key32 {
    let r = [
        le32(&raw[0..4]) & 0x3ff_ffff,
        (le32(&raw[3..7]) >> 2) & 0x3ff_ff03,
        (le32(&raw[6..10]) >> 4) & 0x3ff_c0ff,
        (le32(&raw[9..13]) >> 6) & 0x3f0_3fff,
        (le32(&raw[12..16]) >> 8) & 0x00f_ffff,
    ];
    Poly1305Key32 {
        r,
        s: [r[1] * 5, r[2] * 5, r[3] * 5, r[4] * 5],
    }
}

/// `poly1305_core_blocks()`: absorb whole 16-byte blocks. `hibit` is 1 for
/// full blocks and 0 for a padded final block.
pub fn poly1305_core_blocks(
    state: &mut Poly1305State32,
    key: &Poly1305Key32,
    data: &[u8],
    hibit: u32,
) {
    if data.is_empty() {
        return;
    }
    let hibit = hibit << 24;
    let [r0, r1, r2, r3, r4] = key.r.map(u64::from);
    let [s1, s2, s3, s4] = key.s.map(u64::from);
    let [mut h0, mut h1, mut h2, mut h3, mut h4] = state.h;

    for block in data.chunks_exact(16) {
        h0 = h0.wrapping_add(le32(&block[0..4]) & 0x3ff_ffff);
        h1 = h1.wrapping_add((le32(&block[3..7]) >> 2) & 0x3ff_ffff);
        h2 = h2.wrapping_add((le32(&block[6..10]) >> 4) & 0x3ff_ffff);
        h3 = h3.wrapping_add((le32(&block[9..13]) >> 6) & 0x3ff_ffff);
        h4 = h4.wrapping_add((le32(&block[12..16]) >> 8) | hibit);

        let (h0w, h1w, h2w, h3w, h4w) = (
            u64::from(h0),
            u64::from(h1),
            u64::from(h2),
            u64::from(h3),
            u64::from(h4),
        );
        let d0 = h0w * r0 + h1w * s4 + h2w * s3 + h3w * s2 + h4w * s1;
        let mut d1 = h0w * r1 + h1w * r0 + h2w * s4 + h3w * s3 + h4w * s2;
        let mut d2 = h0w * r2 + h1w * r1 + h2w * r0 + h3w * s4 + h4w * s3;
        let mut d3 = h0w * r3 + h1w * r2 + h2w * r1 + h3w * r0 + h4w * s4;
        let mut d4 = h0w * r4 + h1w * r3 + h2w * r2 + h3w * r1 + h4w * r0;

        // (partial) h %= p
        let mut c = (d0 >> 26) as u32;
        h0 = d0 as u32 & 0x3ff_ffff;
        d1 += u64::from(c);
        c = (d1 >> 26) as u32;
        h1 = d1 as u32 & 0x3ff_ffff;
        d2 += u64::from(c);
        c = (d2 >> 26) as u32;
        h2 = d2 as u32 & 0x3ff_ffff;
        d3 += u64::from(c);
        c = (d3 >> 26) as u32;
        h3 = d3 as u32 & 0x3ff_ffff;
        d4 += u64::from(c);
        c = (d4 >> 26) as u32;
        h4 = d4 as u32 & 0x3ff_ffff;
        h0 = h0.wrapping_add(c.wrapping_mul(5));
        c = h0 >> 26;
        h0 &= 0x3ff_ffff;
        h1 = h1.wrapping_add(c);
    }

    state.h = [h0, h1, h2, h3, h4];
}

/// `poly1305_core_emit()`: fully carry `h`, reduce it below p, and add the
/// nonce when there is one. The tag is the first 16 bytes.
pub fn poly1305_core_emit(state: &Poly1305State32, nonce: Option<[u32; 4]>) -> [u8; 16] {
    let [mut h0, mut h1, mut h2, mut h3, mut h4] = state.h;

    // fully carry h
    let mut c = h1 >> 26;
    h1 &= 0x3ff_ffff;
    h2 = h2.wrapping_add(c);
    c = h2 >> 26;
    h2 &= 0x3ff_ffff;
    h3 = h3.wrapping_add(c);
    c = h3 >> 26;
    h3 &= 0x3ff_ffff;
    h4 = h4.wrapping_add(c);
    c = h4 >> 26;
    h4 &= 0x3ff_ffff;
    h0 = h0.wrapping_add(c.wrapping_mul(5));
    c = h0 >> 26;
    h0 &= 0x3ff_ffff;
    h1 = h1.wrapping_add(c);

    // compute h + -p
    let mut g0 = h0.wrapping_add(5);
    c = g0 >> 26;
    g0 &= 0x3ff_ffff;
    let mut g1 = h1.wrapping_add(c);
    c = g1 >> 26;
    g1 &= 0x3ff_ffff;
    let mut g2 = h2.wrapping_add(c);
    c = g2 >> 26;
    g2 &= 0x3ff_ffff;
    let mut g3 = h3.wrapping_add(c);
    c = g3 >> 26;
    g3 &= 0x3ff_ffff;
    let mut g4 = h4.wrapping_add(c).wrapping_sub(1 << 26);

    // select h if h < p, or h + -p if h >= p
    let mut mask = (g4 >> 31).wrapping_sub(1);
    g0 &= mask;
    g1 &= mask;
    g2 &= mask;
    g3 &= mask;
    g4 &= mask;
    mask = !mask;

    h0 = (h0 & mask) | g0;
    h1 = (h1 & mask) | g1;
    h2 = (h2 & mask) | g2;
    h3 = (h3 & mask) | g3;
    h4 = (h4 & mask) | g4;

    // h = h % (2^128)
    h0 |= h1 << 26;
    h1 = (h1 >> 6) | (h2 << 20);
    h2 = (h2 >> 12) | (h3 << 14);
    h3 = (h3 >> 18) | (h4 << 8);

    let mut words = [h0, h1, h2, h3];
    if let Some(nonce) = nonce {
        // mac = (h + nonce) % (2^128)
        let mut f = u64::from(words[0]) + u64::from(nonce[0]);
        words[0] = f as u32;
        for i in 1..4 {
            f = u64::from(words[i]) + u64::from(nonce[i]) + (f >> 32);
            words[i] = f as u32;
        }
    }

    let mut mac = [0u8; 16];
    for (i, w) in words.iter().enumerate() {
        mac[4 * i..4 * i + 4].copy_from_slice(&w.to_le_bytes());
    }
    mac
}

/// The one-shot tag for a 32-byte key and a message, as `poly1305()` in
/// `lib/crypto/poly1305.c` computes it with this core: full blocks with the
/// high bit set, then a final partial block padded with a 1 byte and no high bit.
pub fn poly1305_tag(key: &[u8; 32], msg: &[u8]) -> [u8; 16] {
    let raw_r: &[u8; 16] = key[..16].try_into().unwrap();
    let k = poly1305_core_setkey(raw_r);
    let mut st = Poly1305State32::default();

    let full = msg.len() - msg.len() % 16;
    poly1305_core_blocks(&mut st, &k, &msg[..full], 1);
    let rem = &msg[full..];
    if !rem.is_empty() {
        let mut last = [0u8; 16];
        last[..rem.len()].copy_from_slice(rem);
        last[rem.len()] = 1;
        poly1305_core_blocks(&mut st, &k, &last, 0);
    }

    let nonce = [
        le32(&key[16..20]),
        le32(&key[20..24]),
        le32(&key[24..28]),
        le32(&key[28..32]),
    ];
    poly1305_core_emit(&st, Some(nonce))
}

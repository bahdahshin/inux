// SPDX-License-Identifier: GPL-2.0
//! Poly1305 one-time authenticator, with the 44-bit limb core.
//!
//! Ported from `lib/crypto/poly1305-donna64.c` (`poly1305_core_setkey()`,
//! `poly1305_core_blocks()`, `poly1305_core_emit()`) and the padding rules of
//! `lib/crypto/poly1305.c`. The field is held in three 44-bit limbs (42 for the
//! top) so the products fit in 128-bit integers, as in the donna code. The
//! architecture versions in C compute the same tag.

/// The clamped key `r`, as three limbs, and `s` times 20 for the reduction.
#[derive(Clone, Copy, Debug)]
pub struct Poly1305Key {
    r: [u64; 3],
    s: [u64; 2],
}

/// The accumulator `h`, three limbs.
#[derive(Clone, Copy, Debug, Default)]
pub struct Poly1305State {
    h: [u64; 3],
}

const MASK44: u64 = 0xfff_ffff_ffff;
const MASK42: u64 = 0x3ff_ffff_ffff;

/// `poly1305_core_setkey()`: clamp `r` from the first 16 key bytes.
pub fn poly1305_core_setkey(raw: &[u8; 16]) -> Poly1305Key {
    let t0 = u64::from_le_bytes(raw[0..8].try_into().unwrap());
    let t1 = u64::from_le_bytes(raw[8..16].try_into().unwrap());
    let r0 = t0 & 0xffc_0fff_ffff;
    let r1 = ((t0 >> 44) | (t1 << 20)) & 0xfff_ffc0_ffff;
    #[allow(clippy::unusual_byte_groupings)]
    let r2 = (t1 >> 24) & 0x00ff_ffff_c0f;
    Poly1305Key {
        r: [r0, r1, r2],
        s: [r1 * 20, r2 * 20],
    }
}

/// `poly1305_core_blocks()`: absorb whole 16-byte blocks. `hibit` is 1 for
/// full blocks (it sets the 2^128 bit) and 0 for a padded final block.
pub fn poly1305_core_blocks(state: &mut Poly1305State, key: &Poly1305Key, data: &[u8], hibit: u64) {
    let hibit64 = hibit << 40;
    let [r0, r1, r2] = key.r;
    let [s1, s2] = key.s;
    let [mut h0, mut h1, mut h2] = state.h;
    for block in data.chunks_exact(16) {
        let t0 = u64::from_le_bytes(block[0..8].try_into().unwrap());
        let t1 = u64::from_le_bytes(block[8..16].try_into().unwrap());
        h0 += t0 & MASK44;
        h1 += ((t0 >> 44) | (t1 << 20)) & MASK44;
        h2 += ((t1 >> 24) & MASK42) | hibit64;

        let mut d0 = u128::from(h0) * u128::from(r0);
        d0 += u128::from(h1) * u128::from(s2);
        d0 += u128::from(h2) * u128::from(s1);
        let mut d1 = u128::from(h0) * u128::from(r1);
        d1 += u128::from(h1) * u128::from(r0);
        d1 += u128::from(h2) * u128::from(s2);
        let mut d2 = u128::from(h0) * u128::from(r2);
        d2 += u128::from(h1) * u128::from(r1);
        d2 += u128::from(h2) * u128::from(r0);

        // Partial reduction modulo 2^130 - 5.
        let mut c = (d0 >> 44) as u64;
        h0 = (d0 as u64) & MASK44;
        d1 += u128::from(c);
        c = (d1 >> 44) as u64;
        h1 = (d1 as u64) & MASK44;
        d2 += u128::from(c);
        c = (d2 >> 42) as u64;
        h2 = (d2 as u64) & MASK42;
        h0 += c * 5;
        c = h0 >> 44;
        h0 &= MASK44;
        h1 += c;
    }
    state.h = [h0, h1, h2];
}

/// `poly1305_core_emit()`: fully reduce `h`, add `s` (as four little-endian
/// 32-bit words), and return the 16-byte tag.
pub fn poly1305_core_emit(state: &Poly1305State, nonce: Option<[u32; 4]>) -> [u8; 16] {
    let [mut h0, mut h1, mut h2] = state.h;
    let mut c = h1 >> 44;
    h1 &= MASK44;
    h2 += c;
    c = h2 >> 42;
    h2 &= MASK42;
    h0 += c * 5;
    c = h0 >> 44;
    h0 &= MASK44;
    h1 += c;
    c = h1 >> 44;
    h1 &= MASK44;
    h2 += c;
    c = h2 >> 42;
    h2 &= MASK42;
    h0 += c * 5;
    c = h0 >> 44;
    h0 &= MASK44;
    h1 += c;

    // Compute h - p and keep it if h >= p.
    let mut g0 = h0 + 5;
    c = g0 >> 44;
    g0 &= MASK44;
    let mut g1 = h1 + c;
    c = g1 >> 44;
    g1 &= MASK44;
    let g2 = (h2 + c).wrapping_sub(1 << 42);
    let mask = (g2 >> 63).wrapping_sub(1);
    g0 &= mask;
    g1 &= mask;
    let g2 = g2 & mask;
    let keep = !mask;
    h0 = (h0 & keep) | g0;
    h1 = (h1 & keep) | g1;
    h2 = (h2 & keep) | g2;

    if let Some(n) = nonce {
        let t0 = (u64::from(n[1]) << 32) | u64::from(n[0]);
        let t1 = (u64::from(n[3]) << 32) | u64::from(n[2]);
        h0 += t0 & MASK44;
        c = h0 >> 44;
        h0 &= MASK44;
        h1 += (((t0 >> 44) | (t1 << 20)) & MASK44) + c;
        c = h1 >> 44;
        h1 &= MASK44;
        h2 += ((t1 >> 24) & MASK42) + c;
        h2 &= MASK42;
    }

    let lo = h0 | (h1 << 44);
    let hi = (h1 >> 20) | (h2 << 24);
    let mut mac = [0u8; 16];
    mac[0..8].copy_from_slice(&lo.to_le_bytes());
    mac[8..16].copy_from_slice(&hi.to_le_bytes());
    mac
}

/// The Poly1305 tag of `msg` under the 32-byte one-time key `key` (`r`
/// followed by `s`), as in RFC 8439.
pub fn poly1305(key: &[u8; 32], msg: &[u8]) -> [u8; 16] {
    let r: [u8; 16] = key[..16].try_into().unwrap();
    let key_r = poly1305_core_setkey(&r);
    let s: [u32; 4] = core::array::from_fn(|i| {
        u32::from_le_bytes(key[16 + 4 * i..20 + 4 * i].try_into().unwrap())
    });

    let mut state = Poly1305State::default();
    let full = msg.len() / 16 * 16;
    poly1305_core_blocks(&mut state, &key_r, &msg[..full], 1);
    let rest = &msg[full..];
    if !rest.is_empty() {
        // The final partial block is padded with a 1 bit, then zeros, and
        // no 2^128 bit is added.
        let mut last = [0u8; 16];
        last[..rest.len()].copy_from_slice(rest);
        last[rest.len()] = 1;
        poly1305_core_blocks(&mut state, &key_r, &last, 0);
    }
    poly1305_core_emit(&state, Some(s))
}

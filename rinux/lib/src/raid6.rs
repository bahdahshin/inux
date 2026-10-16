// SPDX-License-Identifier: GPL-2.0-or-later
//! RAID-6 over GF(2^8): the field tables, the P/Q syndrome, its partial
//! update, and the two-disk and one-disk recovery.
//!
//! Ported from `lib/raid/raid6/mktables.c` (the tables that Linux generates
//! at build time, here `const` items computed by const functions),
//! `lib/raid/raid6/int.uc` (the generic syndrome; the `$#`-way unrolled
//! variants compute the same bytes, so one function serves them all), and
//! `lib/raid/raid6/recov.c` (`raid6_2data_recov()` and `raid6_datap_recov()`).
//!
//! The field is GF(2^8) with reducing polynomial 0x11d, generator 2. The C
//! code works a machine word at a time; this port works a byte at a time,
//! which gives the same bytes.

/// `gfmul()` from `mktables.c`: multiply in GF(2^8).
pub const fn gfmul(a: u8, b: u8) -> u8 {
    let mut a = a;
    let mut b = b;
    let mut v = 0u8;
    while b != 0 {
        if b & 1 != 0 {
            v ^= a;
        }
        a = (a << 1) ^ if a & 0x80 != 0 { 0x1d } else { 0 };
        b >>= 1;
    }
    v
}

/// `gfpow()` from `mktables.c`: `a` raised to `b`, with `b` taken mod 255.
pub const fn gfpow(a: u8, b: i32) -> u8 {
    let mut a = a;
    let mut b = b % 255;
    if b < 0 {
        b += 255;
    }
    let mut b = b as u32;
    let mut v = 1u8;
    while b != 0 {
        if b & 1 != 0 {
            v = gfmul(v, a);
        }
        a = gfmul(a, a);
        b >>= 1;
    }
    v
}

const fn build_exp() -> [u8; 256] {
    let mut t = [0u8; 256];
    let mut v = 1u8;
    let mut i = 0;
    while i < 256 {
        t[i] = v;
        v = gfmul(v, 2);
        if v == 1 {
            // Entry 255 is not a real power; C leaves it zero.
            v = 0;
        }
        i += 1;
    }
    t
}

const fn build_log(exp: &[u8; 256]) -> [u8; 256] {
    let mut t = [255u8; 256];
    let mut k = 0;
    while k < 256 {
        // The first k wins, as C's `break` does.
        let mut j = 0;
        let mut found = false;
        while j < 256 {
            if !found && exp[k] as usize == j && t[j] == 255 {
                t[j] = k as u8;
                found = true;
            }
            j += 1;
        }
        k += 1;
    }
    t
}

const fn build_inv() -> [u8; 256] {
    let mut t = [0u8; 256];
    let mut i = 0;
    while i < 256 {
        t[i] = gfpow(i as u8, 254);
        i += 1;
    }
    t
}

const fn build_exi(exp: &[u8; 256], inv: &[u8; 256]) -> [u8; 256] {
    let mut t = [0u8; 256];
    let mut i = 0;
    while i < 256 {
        t[i] = inv[(exp[i] ^ 1) as usize];
        i += 1;
    }
    t
}

/// `raid6_gfexp`: the powers of 2, `gfexp[i] = 2^i`, with entry 255 zero.
pub const GFEXP: [u8; 256] = build_exp();
/// `raid6_gflog`: discrete log base 2, 255 for zero and unreachable values.
pub const GFLOG: [u8; 256] = build_log(&GFEXP);
/// `raid6_gfinv`: the multiplicative inverse, `x^254`; zero maps to zero.
pub const GFINV: [u8; 256] = build_inv();
/// `raid6_gfexi`: the inverse of `2^x + 1`, `gfexi[x] = inv(2^x ^ 1)`.
pub const GFEXI: [u8; 256] = build_exi(&GFEXP, &GFINV);

/// Multiply every byte by 2 (`SHLBYTE()` with the `MASK()` reduction).
#[inline]
const fn mul2(v: u8) -> u8 {
    (v << 1) ^ if v & 0x80 != 0 { 0x1d } else { 0 }
}

/// `raid6_int*_gen_syndrome()`: the P parity (XOR of every data disk) and the
/// Q syndrome (`sum 2^d * D_d`) of `data`, written to `p` and `q`.
///
/// `data` lists the data disks in order; the Horner walk runs from the
/// highest disk down, as in the C code. All blocks have the same length.
pub fn gen_syndrome(data: &[&[u8]], p: &mut [u8], q: &mut [u8]) {
    let bytes = p.len();
    let Some((&top, rest)) = data.split_last() else {
        p.fill(0);
        q.fill(0);
        return;
    };
    for i in 0..bytes {
        let mut wp = top[i];
        let mut wq = top[i];
        for d in rest.iter().rev() {
            wp ^= d[i];
            wq = mul2(wq) ^ d[i];
        }
        p[i] = wp;
        q[i] = wq;
    }
}

/// `raid6_int*_xor_syndrome()`: add the contribution of data disks
/// `start..=stop` into an existing P and Q, for a partial stripe write.
///
/// `data` lists every data disk; disks outside the range are only shifted
/// for Q, as the C left-side optimization does.
pub fn xor_syndrome(data: &[&[u8]], start: usize, stop: usize, p: &mut [u8], q: &mut [u8]) {
    let bytes = p.len();
    for i in 0..bytes {
        let mut wp = data[stop][i];
        let mut wq = data[stop][i];
        for z in (start..stop).rev() {
            wp ^= data[z][i];
            wq = mul2(wq) ^ data[z][i];
        }
        for _ in 0..start {
            wq = mul2(wq);
        }
        p[i] ^= wp;
        q[i] ^= wq;
    }
}

/// The P and Q syndromes of `data` for one byte, leaving out the disks in
/// `skip` (both are data disks; a skipped disk contributes nothing). Each
/// disk `d` contributes `2^d * D_d` to Q.
fn survivor_syndrome(data: &[&[u8]], skip: [usize; 2], i: usize) -> (u8, u8) {
    let mut sp = 0u8;
    let mut sq = 0u8;
    for (d, block) in data.iter().enumerate() {
        if d == skip[0] || d == skip[1] {
            continue;
        }
        sp ^= block[i];
        sq ^= gfmul(GFEXP[d], block[i]);
    }
    (sp, sq)
}

/// `raid6_2data_recov()`: rebuild two failed data disks `faila < failb`.
///
/// `data` holds every data disk; the entries at `faila` and `failb` are
/// ignored. `p` and `q` are the surviving parity blocks. The recovered
/// blocks are written to `out_a` and `out_b`.
pub fn recov_2data(
    data: &[&[u8]],
    p: &[u8],
    q: &[u8],
    faila: usize,
    failb: usize,
    out_a: &mut [u8],
    out_b: &mut [u8],
) {
    assert!(faila < failb);
    let ga = GFEXP[faila];
    let gb = GFEXP[failb];
    let inv_d = GFINV[(ga ^ gb) as usize];
    for i in 0..p.len() {
        let (sp, sq) = survivor_syndrome(data, [faila, failb], i);
        // px = A ^ B and qx = 2^a A ^ 2^b B, from the parity equations.
        let px = p[i] ^ sp;
        let qx = q[i] ^ sq;
        let b = gfmul(inv_d, qx ^ gfmul(ga, px));
        out_b[i] = b;
        out_a[i] = px ^ b;
    }
}

/// `raid6_datap_recov()`: rebuild one failed data disk `faila` from Q.
///
/// The P parity is regenerated as the XOR of all data disks, including the
/// rebuilt one, and written to `p`.
pub fn recov_datap(data: &[&[u8]], p: &mut [u8], q: &[u8], faila: usize, out_a: &mut [u8]) {
    let inv_a = GFINV[GFEXP[faila] as usize];
    for i in 0..p.len() {
        let (sp, sq) = survivor_syndrome(data, [faila, usize::MAX], i);
        let a = gfmul(inv_a, q[i] ^ sq);
        out_a[i] = a;
        p[i] = sp ^ a;
    }
}

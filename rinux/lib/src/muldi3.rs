// SPDX-License-Identifier: GPL-2.0-only
//! 64-bit multiplication from 32-bit halves, for GCC on 32-bit targets.
//!
//! Ported from `lib/muldi3.c`. The C file builds the full product of two
//! 32-bit values, `umul_ppmm()`, from 16-bit pieces, then adds the two cross
//! products to the high half. It is kept here as the same algorithm, so the
//! results match the C. x86-64 never calls `__muldi3`, and Rust's `*` lowers
//! to one instruction, so these functions exist for the sake of the port.

/// `umul_ppmm()`: the full 64-bit product of two 32-bit values, returned as
/// `(high, low)`. Computed from 16-bit halves as in C.
pub fn umul_ppmm(u: u32, v: u32) -> (u32, u32) {
    const B: u64 = 1 << 16;
    let ul = u & 0xffff;
    let uh = u >> 16;
    let vl = v & 0xffff;
    let vh = v >> 16;

    let x0 = u64::from(ul) * u64::from(vl);
    let mut x1 = u64::from(ul) * u64::from(vh);
    let x2 = u64::from(uh) * u64::from(vl);
    let mut x3 = u64::from(uh) * u64::from(vh);

    x1 += x0 >> 16; // this can't give carry
    x1 += x2; // but this indeed can
    if x1 < x2 {
        x3 += B; // yes, add it in the proper pos
    }

    let w1 = x3 + (x1 >> 16);
    let w0 = (x1 & 0xffff) * B + (x0 & 0xffff);
    (w1 as u32, w0 as u32)
}

/// `__umulsidi3()`: the full product of two 32-bit values.
pub fn umulsidi3(u: u32, v: u32) -> u64 {
    let (high, low) = umul_ppmm(u, v);
    (u64::from(high) << 32) | u64::from(low)
}

/// `__muldi3()`: the low 64 bits of `u * v`, wrapping on overflow.
pub fn muldi3(u: i64, v: i64) -> i64 {
    let (uu, vv) = (u as u64, v as u64);
    let (u_high, u_low) = ((uu >> 32) as u32, uu as u32);
    let (v_high, v_low) = ((vv >> 32) as u32, vv as u32);

    let mut w = umulsidi3(u_low, v_low);
    let cross =
        (u64::from(u_low) * u64::from(v_high)).wrapping_add(u64::from(u_high) * u64::from(v_low));
    // The C high half is 32 bits, so the cross terms are truncated there.
    let high = ((w >> 32) as u32).wrapping_add(cross as u32);
    w = (u64::from(high) << 32) | (w as u32 as u64);
    w as i64
}

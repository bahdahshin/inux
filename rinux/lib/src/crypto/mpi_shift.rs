// SPDX-License-Identifier: GPL-2.0-or-later
//! Bit shifts of a multi-precision number held in 64-bit limbs.
//!
//! Ported from `lib/crypto/mpi/generic_mpih-lshift.c` and
//! `generic_mpih-rshift.c`. Limb 0 is the least significant. Each function
//! shifts by `cnt` bits, `1..=63`, and returns the bits that fell off the end
//! as a limb, as in C. The C functions take a pointer pair and write `wp`
//! while reading `up`; here `res` and `up` are separate slices of the same
//! length, so the in-place use the C code allows is written as one call on
//! a copy.

/// `mpihelp_lshift()`: `res = up << cnt`, with `res.len() == up.len()`.
/// Returns the bits shifted out of the top limb, in the low bits.
///
/// Panics unless `cnt` is in `1..=63` and the lengths match. C leaves a
/// shift count of zero undefined.
pub fn mpihelp_lshift(res: &mut [u64], up: &[u64], cnt: u32) -> u64 {
    assert!((1..64).contains(&cnt), "shift count {cnt} out of range");
    assert_eq!(res.len(), up.len(), "lshift operands differ in size");
    let n = up.len();
    let sh_2 = 64 - cnt;
    let retval = up[n - 1] >> sh_2;
    for j in (1..n).rev() {
        res[j] = (up[j] << cnt) | (up[j - 1] >> sh_2);
    }
    res[0] = up[0] << cnt;
    retval
}

/// `mpihelp_rshift()`: `res = up >> cnt`, with `res.len() == up.len()`.
/// Returns the bits shifted out of the bottom limb, in the high bits.
///
/// Panics unless `cnt` is in `1..=63` and the lengths match.
pub fn mpihelp_rshift(res: &mut [u64], up: &[u64], cnt: u32) -> u64 {
    assert!((1..64).contains(&cnt), "shift count {cnt} out of range");
    assert_eq!(res.len(), up.len(), "rshift operands differ in size");
    let n = up.len();
    let sh_2 = 64 - cnt;
    let retval = up[0] << sh_2;
    for i in 0..n - 1 {
        res[i] = (up[i] >> cnt) | (up[i + 1] << sh_2);
    }
    res[n - 1] = up[n - 1] >> cnt;
    retval
}

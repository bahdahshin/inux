// SPDX-License-Identifier: GPL-2.0
//! 64-bit division and multiply-divide helpers.
//!
//! Ported from `lib/math/div64.c`, the 64-bit half of
//! `include/linux/math64.h` and `include/vdso/math64.h`.
//!
//! Copyright (C) 2003 Bernardo Innocenti <bernie@develer.com>
//!
//! On 64-bit kernels most of these are plain `/` and `%`. They are kept so
//! ported code reads like the C it came from, and because `do_div()`'s
//! in-place convention shows up all over the kernel.
//!
//! Dividing by zero panics, where the C versions trap.

use core::hint::black_box;

/// `div_u64_rem()`: `(dividend / divisor, dividend % divisor)`.
#[inline]
pub const fn div_u64_rem(dividend: u64, divisor: u32) -> (u64, u32) {
    (
        dividend / divisor as u64,
        (dividend % divisor as u64) as u32,
    )
}

/// `div_s64_rem()`: signed `(dividend / divisor, dividend % divisor)`.
#[inline]
pub const fn div_s64_rem(dividend: i64, divisor: i32) -> (i64, i32) {
    (
        dividend / divisor as i64,
        (dividend % divisor as i64) as i32,
    )
}

/// `div64_u64_rem()`: `(dividend / divisor, dividend % divisor)`.
#[inline]
pub const fn div64_u64_rem(dividend: u64, divisor: u64) -> (u64, u64) {
    (dividend / divisor, dividend % divisor)
}

/// `div64_s64_rem()`: signed `(dividend / divisor, dividend % divisor)`.
#[inline]
pub const fn div64_s64_rem(dividend: i64, divisor: i64) -> (i64, i64) {
    (dividend / divisor, dividend % divisor)
}

/// `div64_u64()`: unsigned 64-bit divide.
#[inline]
pub const fn div64_u64(dividend: u64, divisor: u64) -> u64 {
    dividend / divisor
}

/// `div64_s64()`: signed 64-bit divide.
#[inline]
pub const fn div64_s64(dividend: i64, divisor: i64) -> i64 {
    dividend / divisor
}

/// `div_u64()`: unsigned 64-bit by 32-bit divide.
#[inline]
pub const fn div_u64(dividend: u64, divisor: u32) -> u64 {
    div_u64_rem(dividend, divisor).0
}

/// `div_s64()`: signed 64-bit by 32-bit divide.
#[inline]
pub const fn div_s64(dividend: i64, divisor: i32) -> i64 {
    div_s64_rem(dividend, divisor).0
}

/// `div64_ul()`: [`div64_u64`] for `unsigned long`.
#[inline]
pub const fn div64_ul(x: u64, y: u64) -> u64 {
    div64_u64(x, y)
}

/// `div64_long()`: [`div64_s64`] for `long`.
#[inline]
pub const fn div64_long(x: i64, y: i64) -> i64 {
    div64_s64(x, y)
}

/// `do_div()`: divides `n` in place by `base` and returns the remainder.
#[inline]
pub fn do_div(n: &mut u64, base: u32) -> u32 {
    let (q, r) = div_u64_rem(*n, base);
    *n = q;
    r
}

/// `__div64_32()`: the generic shift-and-subtract 64-by-32 divide that
/// 32-bit architectures use for `do_div()`. Divides `n` in place and
/// returns the remainder.
pub fn div64_32(n: &mut u64, base: u32) -> u32 {
    let mut rem = *n;
    let mut b = base as u64;
    let mut res: u64 = 0;
    let mut d: u64 = 1;
    let mut high = (rem >> 32) as u32;

    // Reduce the thing a bit first.
    if high >= base {
        high /= base;
        res = (high as u64) << 32;
        rem -= ((high * base) as u64) << 32;
    }

    while (b as i64) > 0 && b < rem {
        b += b;
        d += d;
    }

    loop {
        if rem >= b {
            rem -= b;
            res += d;
        }
        b >>= 1;
        d >>= 1;
        if d == 0 {
            break;
        }
    }

    *n = res;
    rem as u32
}

/// `iter_div_u64_rem()`: divides by repeated subtraction, for dividends
/// expected to be a small multiple of the divisor. Returns
/// `(quotient, remainder)`.
pub fn iter_div_u64_rem(mut dividend: u64, divisor: u32) -> (u32, u64) {
    let mut ret: u32 = 0;
    while dividend >= divisor as u64 {
        // black_box stops the compiler turning the loop into a modulo,
        // as the empty asm() does in C.
        dividend = black_box(dividend) - divisor as u64;
        ret = ret.wrapping_add(1);
    }
    (ret, dividend)
}

/// `mul_u32_u32()`: 32x32 to 64-bit multiply.
#[inline]
pub const fn mul_u32_u32(a: u32, b: u32) -> u64 {
    a as u64 * b as u64
}

/// `add_u64_u32()`: adds a 32-bit value to a 64-bit one, wrapping.
#[inline]
pub const fn add_u64_u32(a: u64, b: u32) -> u64 {
    a.wrapping_add(b as u64)
}

/// `mul_u64_u32_shr()`: `(a * mul) >> shift` with a 96-bit intermediate.
#[inline]
pub const fn mul_u64_u32_shr(a: u64, mul: u32, shift: u32) -> u64 {
    ((a as u128 * mul as u128) >> shift) as u64
}

/// `mul_u64_u64_shr()`: `(a * mul) >> shift` with a 128-bit intermediate.
#[inline]
pub const fn mul_u64_u64_shr(a: u64, mul: u64, shift: u32) -> u64 {
    ((a as u128 * mul as u128) >> shift) as u64
}

/// `mul_s64_u64_shr()`: signed `(a * b) >> shift`, returned as `u64`.
#[inline]
pub const fn mul_s64_u64_shr(a: i64, b: u64, shift: u32) -> u64 {
    // Take the sign out before multiplying and put it back afterwards.
    let ret = mul_u64_u64_shr(a.unsigned_abs(), b, shift);
    if a < 0 {
        (ret as i64).wrapping_neg() as u64
    } else {
        ret
    }
}

/// `mul_u64_u32_div()`: `(a * mul) / divisor` with a 96-bit intermediate.
/// The result is truncated to 64 bits.
pub fn mul_u64_u32_div(a: u64, mul: u32, divisor: u32) -> u64 {
    let rl = mul_u32_u32(a as u32, mul);
    let mut rh = mul_u32_u32((a >> 32) as u32, mul) + (rl >> 32);

    // Bits 32-63 of the result end up in the low half of rh.
    let rem = do_div(&mut rh, divisor);

    // Bits 0-31 of the result end up in the low half of rl.
    let mut rl = ((rem as u64) << 32) | (rl & 0xffff_ffff);
    do_div(&mut rl, divisor);

    (rh << 32) | (rl & 0xffff_ffff)
}

/// `mul_u64_add_u64_div_u64()`: `(a * b + c) / d` with a 128-bit intermediate.
///
/// Returns `u64::MAX` if the quotient does not fit in 64 bits; x86-64's
/// inline-asm version traps instead. Panics if `d` is 0.
pub fn mul_u64_add_u64_div_u64(a: u64, b: u64, c: u64, d: u64) -> u64 {
    mul_add_div::<32, true>(a, b, c, d)
}

/// `mul_u64_u64_div_u64()`: `(a * b) / d` with a 128-bit intermediate.
#[inline]
pub fn mul_u64_u64_div_u64(a: u64, b: u64, d: u64) -> u64 {
    mul_u64_add_u64_div_u64(a, b, 0, d)
}

/// `mul_u64_u64_div_u64_roundup()`: `(a * b) / d`, rounded up.
#[inline]
pub fn mul_u64_u64_div_u64_roundup(a: u64, b: u64, d: u64) -> u64 {
    mul_u64_add_u64_div_u64(a, b, d.wrapping_sub(1), d)
}

/// `DIV64_U64_ROUND_UP()`.
#[inline]
pub const fn div64_u64_round_up(ll: u64, d: u64) -> u64 {
    div64_u64(ll.wrapping_add(d).wrapping_sub(1), d)
}

/// `DIV_U64_ROUND_UP()`.
#[inline]
pub const fn div_u64_round_up(ll: u64, d: u32) -> u64 {
    div_u64(ll.wrapping_add(d as u64).wrapping_sub(1), d)
}

/// `DIV64_U64_ROUND_CLOSEST()`.
#[inline]
pub const fn div64_u64_round_closest(dividend: u64, divisor: u64) -> u64 {
    div64_u64(dividend.wrapping_add(divisor / 2), divisor)
}

/// `DIV_U64_ROUND_CLOSEST()`.
#[inline]
pub const fn div_u64_round_closest(dividend: u64, divisor: u32) -> u64 {
    div_u64(dividend.wrapping_add(divisor as u64 / 2), divisor)
}

/// `DIV_S64_ROUND_CLOSEST()`.
#[inline]
pub const fn div_s64_round_closest(dividend: i64, divisor: i32) -> i64 {
    if (dividend > 0) == (divisor > 0) {
        div_s64(dividend.wrapping_add(divisor as i64 / 2), divisor)
    } else {
        div_s64(dividend.wrapping_sub(divisor as i64 / 2), divisor)
    }
}

#[inline]
fn mul_add(a: u32, b: u32, c: u32) -> u64 {
    add_u64_u32(mul_u32_u32(a, b), c)
}

/// `a * b + c` as `(high, low)` 64-bit halves. `INT128` picks the native
/// 128-bit multiply; otherwise the product is built from 32-bit pieces, as
/// on 32-bit kernels.
#[inline]
fn mul_u64_u64_add_u64<const INT128: bool>(a: u64, b: u64, c: u64) -> (u64, u64) {
    if INT128 {
        let prod = a as u128 * b as u128 + c as u128;
        return ((prod >> 64) as u64, prod as u64);
    }
    // (x-1)(x-1) + 2(x-1) == x.x - 1, so two u32s can be added to a u64.
    let x = mul_add(a as u32, b as u32, c as u32);
    let y = mul_add(a as u32, (b >> 32) as u32, (c >> 32) as u32);
    let y = add_u64_u32(y, (x >> 32) as u32);
    let z = mul_add((a >> 32) as u32, (b >> 32) as u32, (y >> 32) as u32);
    let y = mul_add((a >> 32) as u32, b as u32, y as u32);
    let lo = (y << 32).wrapping_add(x as u32 as u64);
    (add_u64_u32(z, (y >> 32) as u32), lo)
}

/// `a * b + c` where `b` is an `unsigned long` quotient digit: 64 bits wide
/// with 32-bit iterations, 32 bits wide with 16-bit ones.
#[inline]
fn mul_u64_long_add_u64<const BITS: u32, const INT128: bool>(a: u64, b: u64, c: u64) -> (u64, u64) {
    if BITS == 32 {
        return mul_u64_u64_add_u64::<INT128>(a, b, c);
    }
    let b = b as u32;
    let n_lo = mul_add(a as u32, b, c as u32);
    let n_med = mul_add((a >> 32) as u32, b, (c >> 32) as u32);
    let n_med = add_u64_u32(n_med, (n_lo >> 32) as u32);
    (n_med >> 32, (n_med << 32) | (n_lo as u32 as u64))
}

/// The generic `mul_u64_add_u64_div_u64()` long division, using `BITS`-bit
/// quotient digits: 32 on 64-bit kernels, 16 on 32-bit ones.
fn mul_add_div<const BITS: u32, const INT128: bool>(a: u64, b: u64, c: u64, mut d: u64) -> u64 {
    let (mut n_hi, mut n_lo) = mul_u64_u64_add_u64::<INT128>(a, b, c);

    if n_hi == 0 {
        return div64_u64(n_lo, d);
    }

    if n_hi >= d {
        // Divide by zero traps, as in C.
        if d == 0 {
            return u64::MAX / black_box(0);
        }
        // Overflow: the result does not fit in a u64.
        return u64::MAX;
    }

    // Left-align the divisor, shifting the dividend to match.
    let d_z_hi = d.leading_zeros();
    if d_z_hi != 0 {
        d <<= d_z_hi;
        n_hi = (n_hi << d_z_hi) | (n_lo >> (64 - d_z_hi));
        n_lo <<= d_z_hi;
    }

    let mut reps = 64 / BITS;
    // Fewer iterations for small dividends.
    if (n_hi >> 32) as u32 == 0 {
        reps -= 32 / BITS;
        n_hi = (n_hi << 32) | (n_lo >> 32);
        n_lo <<= 32;
    }
    if BITS == 16 && (n_hi >> 48) as u32 == 0 {
        reps -= 1;
        n_hi = add_u64_u32(n_hi << 16, (n_lo >> 48) as u32);
        n_lo <<= 16;
    }

    // Invert the dividend so the loop can add instead of subtract.
    n_lo = !n_lo;
    n_hi = !n_hi;

    // The top BITS bits of the divisor give a low estimate of each
    // quotient digit.
    let d_msig = (d >> (64 - BITS)) + 1;

    // Long division with BITS-bit digits. The estimate can be low and one
    // bit wider than a digit; the worst case, ~0 / 0x8000, needs two fixups.
    let mut quotient: u64 = 0;
    for _ in 0..reps {
        let mut q_digit = (!n_hi >> (64 - 2 * BITS)) / d_msig;
        // Shift n left to line up with the product q_digit * d.
        let mut overflow = (n_hi >> (64 - BITS)) as u32;
        n_hi = add_u64_u32(n_hi << BITS, (n_lo >> (64 - BITS)) as u32);
        n_lo <<= BITS;
        // Add the product to the negated dividend.
        let (carry, hi) = mul_u64_long_add_u64::<BITS, INT128>(d, q_digit, n_hi);
        n_hi = hi;
        overflow = overflow.wrapping_add(carry as u32);
        // Correct for the estimate being low.
        while overflow < 0xffff_ffff >> (32 - BITS) {
            q_digit += 1;
            n_hi = n_hi.wrapping_add(d);
            overflow = overflow.wrapping_add((n_hi < d) as u32);
        }
        quotient = if BITS == 32 {
            (quotient << BITS).wrapping_add(q_digit)
        } else {
            add_u64_u32(quotient << BITS, q_digit as u32)
        };
    }

    // The remainder cannot overflow, but one more copy of the divisor may
    // still fit.
    if n_hi.wrapping_add(d) > n_hi {
        quotient += 1;
    }
    quotient
}

#[cfg(test)]
mod tests {
    //! Port of `lib/math/test_mul_u64_u64_div_u64.c`. The C test builds
    //! `div64.c` twice to cover both digit sizes; this runs every
    //! combination of digit size and multiply.

    use super::*;

    // a, b, d, a * b / d, and whether rounding up adds one.
    #[rustfmt::skip]
    const TEST_VALUES: &[(u64, u64, u64, u64, u64)] = &[
        // Edge values, then a few random ones.
        (               0xb,                0x7,                0x3,               0x19, 1),
        (        0xffff0000,         0xffff0000,                0xf, 0x1110eeef00000000, 0),
        (        0xffffffff,         0xffffffff,                0x1, 0xfffffffe00000001, 0),
        (        0xffffffff,         0xffffffff,                0x2, 0x7fffffff00000000, 1),
        (       0x1ffffffff,         0xffffffff,                0x2, 0xfffffffe80000000, 1),
        (       0x1ffffffff,         0xffffffff,                0x3, 0xaaaaaaa9aaaaaaab, 0),
        (       0x1ffffffff,        0x1ffffffff,                0x4, 0xffffffff00000000, 1),
        (0xffff000000000000, 0xffff000000000000, 0xffff000000000001, 0xfffeffffffffffff, 1),
        (0x3333333333333333, 0x3333333333333333, 0x5555555555555555, 0x1eb851eb851eb851, 1),
        (0x7fffffffffffffff,                0x2,                0x3, 0x5555555555555554, 1),
        (0xffffffffffffffff,                0x2, 0x8000000000000000,                0x3, 1),
        (0xffffffffffffffff,                0x2, 0xc000000000000000,                0x2, 1),
        (0xffffffffffffffff, 0x4000000000000004, 0x8000000000000000, 0x8000000000000007, 1),
        (0xffffffffffffffff, 0x4000000000000001, 0x8000000000000000, 0x8000000000000001, 1),
        (0xffffffffffffffff, 0x8000000000000001, 0xffffffffffffffff, 0x8000000000000001, 0),
        (0xfffffffffffffffe, 0x8000000000000001, 0xffffffffffffffff, 0x8000000000000000, 1),
        (0xffffffffffffffff, 0x8000000000000001, 0xfffffffffffffffe, 0x8000000000000001, 1),
        (0xffffffffffffffff, 0x8000000000000001, 0xfffffffffffffffd, 0x8000000000000002, 1),
        (0x7fffffffffffffff, 0xffffffffffffffff, 0xc000000000000000, 0xaaaaaaaaaaaaaaa8, 1),
        (0xffffffffffffffff, 0x7fffffffffffffff, 0xa000000000000000, 0xccccccccccccccca, 1),
        (0xffffffffffffffff, 0x7fffffffffffffff, 0x9000000000000000, 0xe38e38e38e38e38b, 1),
        (0x7fffffffffffffff, 0x7fffffffffffffff, 0x5000000000000000, 0xccccccccccccccc9, 1),
        (0xffffffffffffffff, 0xfffffffffffffffe, 0xffffffffffffffff, 0xfffffffffffffffe, 0),
        (0xe6102d256d7ea3ae, 0x70a77d0be4c31201, 0xd63ec35ab3220357, 0x78f8bf8cc86c6e18, 1),
        (0xf53bae05cb86c6e1, 0x3847b32d2f8d32e0, 0xcfd4f55a647f403c, 0x42687f79d8998d35, 1),
        (0x9951c5498f941092, 0x1f8c8bfdf287a251, 0xa3c8dc5f81ea3fe2, 0x1d887cb25900091f, 1),
        (0x374fee9daa1bb2bb, 0x0d0bfbff7b8ae3ef, 0xc169337bd42d5179, 0x03bb2dbaffcbb961, 1),
        (0xeac0d03ac10eeaf0, 0x89be05dfa162ed9b, 0x92bb1679a41f0e4b, 0xdc5f5cc9e270d216, 1),
    ];

    fn run(name: &str, f: fn(u64, u64, u64, u64) -> u64) {
        for &(a, b, d, result, round_up) in TEST_VALUES {
            assert_eq!(f(a, b, 0, d), result, "{name}: {a:#x} * {b:#x} / {d:#x}");
            assert_eq!(
                f(a, b, d - 1, d),
                result + round_up,
                "{name}: {a:#x} * {b:#x} +/ {d:#x}"
            );
        }
    }

    #[test]
    fn mul_u64_u64_div_u64_table() {
        for &(a, b, d, result, round_up) in TEST_VALUES {
            assert_eq!(mul_u64_u64_div_u64(a, b, d), result);
            assert_eq!(mul_u64_u64_div_u64_roundup(a, b, d), result + round_up);
        }
        run("64-bit digits, int128", mul_add_div::<32, true>);
        run("64-bit digits, 32-bit multiply", mul_add_div::<32, false>);
        run("32-bit digits, 32-bit multiply", mul_add_div::<16, false>);
        run("32-bit digits, int128", mul_add_div::<16, true>);
    }

    #[test]
    fn mul_add_div_matches_u128() {
        let mut x: u64 = 0x2545_f491_4f6c_dd1d;
        let mut next = || {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            x
        };
        for _ in 0..20_000 {
            let (a, b, c) = (
                next() >> (next() % 64),
                next() >> (next() % 64),
                next() >> (next() % 64),
            );
            let d = (next() >> (next() % 64)).max(1);
            let exact = (a as u128 * b as u128 + c as u128) / d as u128;
            let want = u64::try_from(exact).unwrap_or(u64::MAX);
            assert_eq!(mul_add_div::<32, true>(a, b, c, d), want);
            assert_eq!(mul_add_div::<32, false>(a, b, c, d), want);
            assert_eq!(mul_add_div::<16, false>(a, b, c, d), want);
        }
    }

    #[test]
    fn div64_32_matches_division() {
        let mut x: u64 = 0x9e37_79b9_7f4a_7c15;
        for _ in 0..20_000 {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            let base = ((x >> 17) as u32 >> (x % 32)).max(1);
            let mut n = x;
            let rem = div64_32(&mut n, base);
            assert_eq!((n, rem), (x / base as u64, (x % base as u64) as u32));
        }
    }

    #[test]
    fn mul_u64_u32_div_matches_u128() {
        let mut x: u64 = 0xdead_beef_cafe_f00d;
        for _ in 0..20_000 {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            let mul = (x >> 7) as u32;
            let div = ((x >> 23) as u32).max(1);
            let want = ((x as u128 * mul as u128) / div as u128) as u64;
            assert_eq!(mul_u64_u32_div(x, mul, div), want);
        }
    }
}

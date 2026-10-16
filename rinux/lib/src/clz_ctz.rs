// SPDX-License-Identifier: GPL-2.0-only
//! Count leading and trailing zero bits, for the GCC builtins.
//!
//! Ported from `lib/clz_ctz.c`. GCC emits calls to `__clzsi2` and friends
//! when a target lacks the instruction; they are not called from kernel C.
//! Rust's `leading_zeros()` and `trailing_zeros()` already compile to the
//! hardware instruction, so nothing calls these functions. They are ported
//! so every file in `lib/` has a counterpart.
//!
//! C's `__ffs(0)` is undefined. `trailing_zeros()` returns the bit width for
//! zero, and so do these functions, which is the value GCC's builtins give.

/// `__clzsi2()`: leading zero bits of a 32-bit value. `32 - fls(val)` in C.
pub const fn clzsi2(val: i32) -> i32 {
    (val as u32).leading_zeros() as i32
}

/// `__ctzsi2()`: trailing zero bits of a 32-bit value. `__ffs(val)` in C.
pub const fn ctzsi2(val: i32) -> i32 {
    (val as u32).trailing_zeros() as i32
}

/// `__clzdi2()`: leading zero bits of a 64-bit value. `64 - fls64(val)`.
pub const fn clzdi2(val: u64) -> i32 {
    val.leading_zeros() as i32
}

/// `__ctzdi2()`: trailing zero bits of a 64-bit value. `__ffs64(val)`.
pub const fn ctzdi2(val: u64) -> i32 {
    val.trailing_zeros() as i32
}

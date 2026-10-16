// SPDX-License-Identifier: GPL-2.0
//! Bit numbers, masks and single-bit operations.
//!
//! Ported from `include/linux/bitops.h`, `include/linux/bits.h`,
//! `include/vdso/bits.h`, `include/asm-generic/bitops/generic-non-atomic.h`
//! and the word helpers in `include/asm-generic/bitops/` (`__ffs.h`,
//! `__fls.h`, `ffs.h`, `fls.h`, `fls64.h`, `ffz.h`).
//!
//! A bitmap is a `[u64]` slice: bit `n` lives in word `n / 64` at position
//! `n % 64`, like `unsigned long` bitmaps on 64-bit Linux. Bit numbers and
//! sizes are `usize`.
//!
//! The single-bit operations are the non-atomic `__set_bit()` family. A
//! `&mut` bitmap is not shared, so they are also what `set_bit()` does on
//! it. Atomic bitops on shared bitmaps will come with SMP support.

/// `BITS_PER_LONG`: rinux targets 64-bit machines.
pub const BITS_PER_LONG: usize = 64;
/// `BITS_PER_LONG_LONG`.
pub const BITS_PER_LONG_LONG: usize = 64;
/// `BITS_PER_BYTE`.
pub const BITS_PER_BYTE: usize = 8;

/// `BIT(nr)`: a word with only bit `nr` set.
#[inline]
pub const fn bit(nr: u32) -> u64 {
    1 << nr
}

/// `BIT_ULL(nr)`: same as [`bit`] on 64-bit machines.
#[inline]
pub const fn bit_ull(nr: u32) -> u64 {
    1 << nr
}

/// `BIT_MASK(nr)`: the mask of bit `nr` within its word.
#[inline]
pub const fn bit_mask(nr: usize) -> u64 {
    1 << (nr % BITS_PER_LONG)
}

/// `BIT_WORD(nr)`: index of the word that holds bit `nr`.
#[inline]
pub const fn bit_word(nr: usize) -> usize {
    nr / BITS_PER_LONG
}

/// `GENMASK(h, l)`: bits `l` through `h` set, `h >= l`.
#[inline]
pub const fn genmask(h: u32, l: u32) -> u64 {
    (!0u64 << l) & (!0u64 >> (63 - h))
}

/// `GENMASK_ULL(h, l)`: same as [`genmask`] on 64-bit machines.
#[inline]
pub const fn genmask_ull(h: u32, l: u32) -> u64 {
    genmask(h, l)
}

/// `BIT_ULL_MASK(nr)`.
#[inline]
pub const fn bit_ull_mask(nr: usize) -> u64 {
    1 << (nr % BITS_PER_LONG_LONG)
}

/// `BIT_ULL_WORD(nr)`.
#[inline]
pub const fn bit_ull_word(nr: usize) -> usize {
    nr / BITS_PER_LONG_LONG
}

macro_rules! typed_bits {
    ($($(#[$bdoc:meta])* $bit:ident, $(#[$gdoc:meta])* $genmask:ident: $t:ty;)*) => {
        $(
            $(#[$bdoc])*
            #[inline]
            pub const fn $bit(nr: u32) -> $t {
                1 << nr
            }

            $(#[$gdoc])*
            #[inline]
            pub const fn $genmask(h: u32, l: u32) -> $t {
                (!0 << l) & (!0 >> (<$t>::BITS - 1 - h))
            }
        )*
    };
}

typed_bits! {
    /// `BIT_U8(nr)`.
    bit_u8,
    /// `GENMASK_U8(h, l)`.
    genmask_u8: u8;
    /// `BIT_U16(nr)`.
    bit_u16,
    /// `GENMASK_U16(h, l)`.
    genmask_u16: u16;
    /// `BIT_U32(nr)`.
    bit_u32,
    /// `GENMASK_U32(h, l)`.
    genmask_u32: u32;
    /// `BIT_U64(nr)`.
    bit_u64,
    /// `GENMASK_U64(h, l)`.
    genmask_u64: u64;
    /// `BIT(nr)` for `u128`; C has no `BIT_U128()`.
    bit_u128,
    /// `GENMASK_U128(h, l)`.
    genmask_u128: u128;
}

/// `BITS_PER_TYPE(type)`.
#[inline]
pub const fn bits_per_type<T>() -> usize {
    core::mem::size_of::<T>() * BITS_PER_BYTE
}

/// `BITS_TO_LONGS(nr)`: words needed for `nr` bits.
#[inline]
pub const fn bits_to_longs(nr: usize) -> usize {
    nr.div_ceil(BITS_PER_LONG)
}

/// `BITS_TO_U64(nr)`.
#[inline]
pub const fn bits_to_u64(nr: usize) -> usize {
    nr.div_ceil(64)
}

/// `BITS_TO_U32(nr)`.
#[inline]
pub const fn bits_to_u32(nr: usize) -> usize {
    nr.div_ceil(32)
}

/// `BITS_TO_BYTES(nr)`.
#[inline]
pub const fn bits_to_bytes(nr: usize) -> usize {
    nr.div_ceil(8)
}

/// `__set_bit()`: sets bit `nr`.
#[inline]
pub fn set_bit(nr: usize, addr: &mut [u64]) {
    addr[bit_word(nr)] |= bit_mask(nr);
}

/// `__clear_bit()`: clears bit `nr`.
#[inline]
pub fn clear_bit(nr: usize, addr: &mut [u64]) {
    addr[bit_word(nr)] &= !bit_mask(nr);
}

/// `__change_bit()`: flips bit `nr`.
#[inline]
pub fn change_bit(nr: usize, addr: &mut [u64]) {
    addr[bit_word(nr)] ^= bit_mask(nr);
}

/// `__test_and_set_bit()`: sets bit `nr` and returns its old value.
#[inline]
pub fn test_and_set_bit(nr: usize, addr: &mut [u64]) -> bool {
    let mask = bit_mask(nr);
    let p = &mut addr[bit_word(nr)];
    let old = *p;
    *p = old | mask;
    old & mask != 0
}

/// `__test_and_clear_bit()`: clears bit `nr` and returns its old value.
#[inline]
pub fn test_and_clear_bit(nr: usize, addr: &mut [u64]) -> bool {
    let mask = bit_mask(nr);
    let p = &mut addr[bit_word(nr)];
    let old = *p;
    *p = old & !mask;
    old & mask != 0
}

/// `__test_and_change_bit()`: flips bit `nr` and returns its old value.
#[inline]
pub fn test_and_change_bit(nr: usize, addr: &mut [u64]) -> bool {
    let mask = bit_mask(nr);
    let p = &mut addr[bit_word(nr)];
    let old = *p;
    *p = old ^ mask;
    old & mask != 0
}

/// `test_bit()`: is bit `nr` set?
#[inline]
pub fn test_bit(nr: usize, addr: &[u64]) -> bool {
    (addr[bit_word(nr)] >> (nr & (BITS_PER_LONG - 1))) & 1 != 0
}

/// `__assign_bit()`: sets or clears bit `nr`.
#[inline]
pub fn assign_bit(nr: usize, addr: &mut [u64], value: bool) {
    if value {
        set_bit(nr, addr)
    } else {
        clear_bit(nr, addr)
    }
}

/// `__ffs()`: index of the lowest set bit. Undefined in C for 0; here 64.
#[inline]
pub const fn __ffs(word: u64) -> u32 {
    word.trailing_zeros()
}

/// `__ffs64()`: same as [`__ffs`] on 64-bit machines.
#[inline]
pub const fn __ffs64(word: u64) -> u32 {
    word.trailing_zeros()
}

/// `__fls()`: index of the highest set bit. Undefined in C for 0; here
/// `u32::MAX`.
#[inline]
pub const fn __fls(word: u64) -> u32 {
    63u32.wrapping_sub(word.leading_zeros())
}

/// `ffz()`: index of the lowest clear bit. Undefined in C for `!0`.
#[inline]
pub const fn ffz(word: u64) -> u32 {
    __ffs(!word)
}

/// `ffs()`: one-based index of the lowest set bit, 0 if none (libc style).
#[inline]
pub const fn ffs(x: i32) -> i32 {
    if x == 0 {
        0
    } else {
        x.trailing_zeros() as i32 + 1
    }
}

/// `fls()`: one-based index of the highest set bit, 0 if none.
#[inline]
pub const fn fls(x: u32) -> u32 {
    32 - x.leading_zeros()
}

/// `fls64()`: [`fls`] for 64-bit words.
#[inline]
pub const fn fls64(x: u64) -> u32 {
    64 - x.leading_zeros()
}

/// `fls_long()`: [`fls64`] on 64-bit machines.
#[inline]
pub const fn fls_long(l: u64) -> u32 {
    fls64(l)
}

/// `get_bitmask_order()`: same as [`fls`].
#[inline]
pub const fn get_bitmask_order(count: u32) -> i32 {
    fls(count) as i32
}

/// `get_count_order()`: order of `count` rounded up to a power of two,
/// -1 for 0.
#[inline]
pub const fn get_count_order(count: u32) -> i32 {
    if count == 0 {
        return -1;
    }
    fls(count - 1) as i32
}

/// `get_count_order_long()`: [`get_count_order`] for `u64`.
#[inline]
pub const fn get_count_order_long(l: u64) -> i32 {
    if l == 0 {
        return -1;
    }
    fls_long(l - 1) as i32
}

/// `hweight_long()`: number of set bits.
#[inline]
pub const fn hweight_long(w: u64) -> u32 {
    w.count_ones()
}

/// `fns()`: index of the `n`-th (from 0) set bit of `word`, or 64.
#[inline]
pub const fn fns(mut word: u64, mut n: u32) -> u32 {
    while word != 0 && n != 0 {
        word &= word - 1;
        n -= 1;
    }
    if word != 0 {
        __ffs(word)
    } else {
        BITS_PER_LONG as u32
    }
}

/// `parity8()`: 1 if `val` has an odd number of set bits.
#[inline]
pub const fn parity8(mut val: u8) -> i32 {
    val ^= val >> 4;
    (0x6996 >> (val & 0xf)) & 1
}

/// `sign_extend32()`: sign-extends `value` from bit `index`.
#[inline]
pub const fn sign_extend32(value: u32, index: u32) -> i32 {
    let shift = 31 - index;
    ((value << shift) as i32) >> shift
}

/// `sign_extend64()`: sign-extends `value` from bit `index`.
#[inline]
pub const fn sign_extend64(value: u64, index: u32) -> i64 {
    let shift = 63 - index;
    ((value << shift) as i64) >> shift
}

/// `rol64()`.
#[inline]
pub const fn rol64(word: u64, shift: u32) -> u64 {
    word.rotate_left(shift & 63)
}

/// `ror64()`.
#[inline]
pub const fn ror64(word: u64, shift: u32) -> u64 {
    word.rotate_right(shift & 63)
}

/// `rol32()`.
#[inline]
pub const fn rol32(word: u32, shift: u32) -> u32 {
    word.rotate_left(shift & 31)
}

/// `ror32()`.
#[inline]
pub const fn ror32(word: u32, shift: u32) -> u32 {
    word.rotate_right(shift & 31)
}

/// `rol16()`.
#[inline]
pub const fn rol16(word: u16, shift: u32) -> u16 {
    word.rotate_left(shift & 15)
}

/// `ror16()`.
#[inline]
pub const fn ror16(word: u16, shift: u32) -> u16 {
    word.rotate_right(shift & 15)
}

/// `rol8()`.
#[inline]
pub const fn rol8(word: u8, shift: u32) -> u8 {
    word.rotate_left(shift & 7)
}

/// `ror8()`.
#[inline]
pub const fn ror8(word: u8, shift: u32) -> u8 {
    word.rotate_right(shift & 7)
}

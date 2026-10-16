// SPDX-License-Identifier: GPL-2.0
//! Fast multiplicative hashing of integers and pointers.
//!
//! Ported from `include/linux/hash.h`. (C) 2002 Nadia Yvette Chambers, IBM.
//! The C header has no SPDX line; it is covered by the kernel's GPL-2.0.
//!
//! The hash multiplies by a large odd number and keeps the high bits, which
//! are the ones every input bit affects. The multiplier is the negative of
//! the golden ratio, `(1 - phi) = phi**2 = (3 - sqrt(5))/2`, scaled to the
//! word size (Knuth vol 3, section 6.4, exercise 9).

/// `GOLDEN_RATIO_32`.
pub const GOLDEN_RATIO_32: u32 = 0x61C88647;
/// `GOLDEN_RATIO_64`.
pub const GOLDEN_RATIO_64: u64 = 0x61C8864680B583EB;
/// `GOLDEN_RATIO_PRIME`: not prime any more, but the name remains.
pub const GOLDEN_RATIO_PRIME: u64 = GOLDEN_RATIO_64;

/// `__hash_32()`.
#[inline]
pub const fn __hash_32(val: u32) -> u32 {
    val.wrapping_mul(GOLDEN_RATIO_32)
}

/// `hash_32()`: a `bits`-bit hash of `val`, `1 <= bits <= 32`.
#[inline]
pub const fn hash_32(val: u32, bits: u32) -> u32 {
    // High bits are more random, so use them.
    __hash_32(val) >> (32 - bits)
}

/// `hash_64()`: a `bits`-bit hash of `val`, `1 <= bits <= 32`.
#[inline]
pub const fn hash_64(val: u64, bits: u32) -> u32 {
    // 64x64-bit multiply is efficient on all 64-bit processors.
    (val.wrapping_mul(GOLDEN_RATIO_64) >> (64 - bits)) as u32
}

/// `hash_long()`: [`hash_64`] on 64-bit machines.
#[inline]
pub const fn hash_long(val: u64, bits: u32) -> u32 {
    hash_64(val, bits)
}

/// `hash_ptr()`: hashes a pointer's address.
#[inline]
pub fn hash_ptr<T: ?Sized>(ptr: *const T, bits: u32) -> u32 {
    hash_long(ptr.cast::<()>() as usize as u64, bits)
}

/// `hash32_ptr()`: folds a pointer's address to 32 bits. No hashing to
/// speak of.
#[inline]
pub fn hash32_ptr<T: ?Sized>(ptr: *const T) -> u32 {
    let val = ptr.cast::<()>() as usize as u64;
    (val ^ (val >> 32)) as u32
}

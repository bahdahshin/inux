// SPDX-License-Identifier: GPL-2.0 OR MIT
//! Whether an integer type is signed.
//!
//! Ported from `is_signed_type()` in `include/linux/compiler.h`. The C macro
//! asks the compiler whether `(type)-1 < (type)0`, which covers integer types,
//! enums and pointers alike. Rust has no such operator, so each type states
//! its answer through [`IsSignedType`]: the integers, `bool` and `char` are
//! implemented here, raw pointers below, and a user type implements the
//! trait for itself, as an enum with a signed representation would.

/// `is_signed_type(T)`: `true` when `T` is a signed integer type.
pub trait IsSignedType {
    /// The answer for `Self`, as a constant.
    const IS_SIGNED: bool;
}

/// `is_signed_type(x)` for a value: the answer for its type.
pub const fn is_signed_type<T: IsSignedType>() -> bool {
    T::IS_SIGNED
}

macro_rules! impl_is_signed {
    (signed: $($t:ty),* ; unsigned: $($u:ty),*) => {
        $(
            impl IsSignedType for $t {
                const IS_SIGNED: bool = true;
            }
        )*
        $(
            impl IsSignedType for $u {
                const IS_SIGNED: bool = false;
            }
        )*
    };
}

impl_is_signed!(
    signed: i8, i16, i32, i64, i128, isize;
    unsigned: u8, u16, u32, u64, u128, usize, bool, char
);

impl<T: ?Sized> IsSignedType for *const T {
    const IS_SIGNED: bool = false;
}

impl<T: ?Sized> IsSignedType for *mut T {
    const IS_SIGNED: bool = false;
}

// SPDX-License-Identifier: GPL-2.0 OR MIT
//! Port of `lib/tests/is_signed_type_kunit.c`.
//!
//! The C test checks the `is_signed_type()` macro on each C type. Here the
//! same table runs through [`IsSignedType`]. The C enums are modelled as Rust
//! enums with a `repr` that matches their C signedness, and implement the
//! trait themselves.

use rinux_lib::is_signed_type::{IsSignedType, is_signed_type};

/// `enum unsigned_enum`: all values non-negative, so the C enum is unsigned.
#[repr(u32)]
#[allow(dead_code)]
enum UnsignedEnum {
    ConstantA = 3,
}

/// `enum signed_enum`: holds a negative value, so the C enum is signed.
#[repr(i32)]
#[allow(dead_code)]
enum SignedEnum {
    ConstantB = -1,
    ConstantC = 2,
}

impl IsSignedType for UnsignedEnum {
    const IS_SIGNED: bool = false;
}

impl IsSignedType for SignedEnum {
    const IS_SIGNED: bool = true;
}

#[test]
fn is_signed_type_matches_the_c_table() {
    assert!(!is_signed_type::<bool>());
    assert!(is_signed_type::<i8>());
    assert!(!is_signed_type::<u8>());
    assert!(!is_signed_type::<char>());
    assert!(is_signed_type::<i32>());
    assert!(!is_signed_type::<u32>());
    assert!(is_signed_type::<i64>());
    assert!(!is_signed_type::<u64>());
    assert!(!is_signed_type::<UnsignedEnum>());
    assert!(is_signed_type::<SignedEnum>());
    assert!(!is_signed_type::<*mut u8>());
    assert!(!is_signed_type::<*const u8>());
}

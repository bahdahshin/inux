// SPDX-License-Identifier: GPL-2.0
//! Host tests for the `kstrto*_from_user()` helpers (`lib/kstrtox.c`) over
//! the flat user memory in `lib/src/uaccess.rs`. Linux has no KUnit file for
//! these; the cases follow the copy and parse rules in the C code.

use rinux_lib::errno::{EFAULT, EINVAL};
use rinux_lib::kstrtox::{
    kstrtobool_from_user, kstrtoint_from_user, kstrtol_from_user, kstrtoll_from_user,
    kstrtouint_from_user, kstrtoul_from_user, kstrtoull_from_user,
};
use rinux_lib::uaccess::FlatUser;

const BASE: usize = 0x1000;

/// Maps `text` at `BASE` and runs `f` on it.
fn with_user<T>(text: &[u8], f: impl FnOnce(&FlatUser<'_>) -> T) -> T {
    let mut mem = text.to_vec();
    let user = FlatUser::new(&mut mem, BASE);
    f(&user)
}

#[test]
fn unsigned_parses_up_to_count_bytes() {
    with_user(b"42", |u| {
        assert_eq!(kstrtoull_from_user(u, BASE, 2, 10), Ok(42))
    });
    // The count cuts the string: "123456" with count 3 is "123".
    with_user(b"123456", |u| {
        assert_eq!(kstrtoull_from_user(u, BASE, 3, 10), Ok(123))
    });
}

#[test]
fn a_trailing_newline_is_allowed() {
    with_user(b"7\n", |u| {
        assert_eq!(kstrtouint_from_user(u, BASE, 2, 10), Ok(7))
    });
}

#[test]
fn signed_variants_take_a_minus_and_unsigned_refuse_it() {
    with_user(b"-5", |u| {
        assert_eq!(kstrtoll_from_user(u, BASE, 2, 10), Ok(-5))
    });
    with_user(b"-5", |u| {
        assert_eq!(kstrtol_from_user(u, BASE, 2, 10), Ok(-5))
    });
    with_user(b"-5", |u| {
        assert_eq!(kstrtoint_from_user(u, BASE, 2, 10), Ok(-5))
    });
    with_user(b"-5", |u| {
        assert_eq!(kstrtoull_from_user(u, BASE, 2, 10), Err(EINVAL))
    });
    with_user(b"-5", |u| {
        assert_eq!(kstrtoul_from_user(u, BASE, 2, 10), Err(EINVAL))
    });
    with_user(b"-5", |u| {
        assert_eq!(kstrtouint_from_user(u, BASE, 2, 10), Err(EINVAL))
    });
}

#[test]
fn radix_zero_autodetects_from_the_copied_text() {
    with_user(b"0x1f", |u| {
        assert_eq!(kstrtoull_from_user(u, BASE, 4, 0), Ok(31))
    });
}

#[test]
fn a_string_that_does_not_parse_is_einval() {
    with_user(b"12x", |u| {
        assert_eq!(kstrtoull_from_user(u, BASE, 3, 10), Err(EINVAL))
    });
}

#[test]
fn source_outside_the_mapping_is_efault() {
    with_user(b"1", |u| {
        assert_eq!(kstrtoull_from_user(u, 0x9000, 1, 10), Err(EFAULT))
    });
}

#[test]
fn a_copy_that_runs_off_the_mapping_is_efault() {
    // Two bytes are mapped, but the copy asks for four.
    with_user(b"12", |u| {
        assert_eq!(kstrtoull_from_user(u, BASE, 4, 10), Err(EFAULT))
    });
}

#[test]
fn booleans_read_the_common_words() {
    with_user(b"yes", |u| {
        assert_eq!(kstrtobool_from_user(u, BASE, 3), Ok(true))
    });
    with_user(b"off", |u| {
        assert_eq!(kstrtobool_from_user(u, BASE, 3), Ok(false))
    });
    with_user(b"maybe", |u| {
        assert_eq!(kstrtobool_from_user(u, BASE, 5), Err(EINVAL))
    });
}

// SPDX-License-Identifier: GPL-2.0
//! Tests for the saturating reference count.

use rinux_lib::refcount::{REFCOUNT_MAX, REFCOUNT_SATURATED, Refcount};

#[test]
fn basic_inc_dec() {
    let r = Refcount::new(1);
    assert_eq!(r.read(), 1);
    r.inc();
    assert_eq!(r.read(), 2);
    r.add(3);
    assert_eq!(r.read(), 5);
    // 5 - 2 = 3, not zero.
    assert!(!r.sub_and_test(2));
    assert_eq!(r.read(), 3);
    // 3 - 3 = 0: the last reference.
    assert!(r.sub_and_test(3));
    assert_eq!(r.read(), 0);
}

#[test]
fn dec_and_test() {
    let r = Refcount::new(2);
    assert!(!r.dec_and_test()); // -> 1
    assert!(r.dec_and_test()); // -> 0, last ref
}

#[test]
fn inc_not_zero() {
    let r = Refcount::new(0);
    assert!(!r.inc_not_zero()); // stays 0
    assert_eq!(r.read(), 0);
    let r = Refcount::new(2);
    assert!(r.inc_not_zero());
    assert_eq!(r.read(), 3);
}

#[test]
fn saturate_on_inc_from_zero() {
    // Incrementing a zero refcount is a use-after-free; it saturates.
    let r = Refcount::new(0);
    r.inc();
    assert_eq!(r.read(), REFCOUNT_SATURATED);
    // Once saturated it stays put.
    r.inc();
    assert_eq!(r.read(), REFCOUNT_SATURATED);
}

#[test]
fn saturate_on_overflow() {
    let r = Refcount::new(REFCOUNT_MAX);
    r.inc();
    assert_eq!(r.read(), REFCOUNT_SATURATED);
}

#[test]
fn saturate_on_underflow() {
    // Subtracting more than is held underflows and saturates.
    let r = Refcount::new(1);
    assert!(!r.sub_and_test(2));
    assert_eq!(r.read(), REFCOUNT_SATURATED);
}

#[test]
fn dec_hitting_zero_is_a_leak() {
    // A plain dec that reaches zero is a misuse (should have been
    // dec_and_test), so it saturates.
    let r = Refcount::new(1);
    r.dec();
    assert_eq!(r.read(), REFCOUNT_SATURATED);

    // A dec with references to spare just decrements.
    let r = Refcount::new(3);
    r.dec();
    assert_eq!(r.read(), 2);
}

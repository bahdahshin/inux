// SPDX-License-Identifier: GPL-2.0
//! Tests for the 64-bit atomics, in the spirit of `lib/atomic64_test.c`:
//! every operation's result is checked against plain arithmetic.

use rinux_lib::atomic64::Atomic64;
use std::sync::Arc;
use std::thread;

#[test]
fn set_and_read() {
    let v = Atomic64::new(0);
    assert_eq!(v.read(), 0);
    v.set(i64::MIN);
    assert_eq!(v.read(), i64::MIN);
}

#[test]
fn arithmetic_returns_new_and_old_values() {
    let v = Atomic64::new(10);
    v.add(5);
    assert_eq!(v.read(), 15);
    v.sub(20);
    assert_eq!(v.read(), -5);
    assert_eq!(v.add_return(7), 2);
    assert_eq!(v.sub_return(2), 0);
    assert_eq!(v.fetch_add(3), 0);
    assert_eq!(v.fetch_sub(1), 3);
    assert_eq!(v.read(), 2);
}

#[test]
fn inc_and_dec() {
    let v = Atomic64::new(0);
    v.inc();
    v.inc();
    v.dec();
    assert_eq!(v.read(), 1);
}

#[test]
fn bit_operations_fetch_old_value() {
    let v = Atomic64::new(0b1100);
    v.and(0b1010);
    assert_eq!(v.read(), 0b1000);
    v.or(0b0011);
    assert_eq!(v.read(), 0b1011);
    v.xor(0b1111);
    assert_eq!(v.read(), 0b0100);
    assert_eq!(v.fetch_or(0b0001), 0b0100);
    assert_eq!(v.fetch_and(0b0110), 0b0101);
    assert_eq!(v.fetch_xor(0b1000), 0b0100);
    assert_eq!(v.read(), 0b1100);
}

#[test]
fn dec_if_positive_only_decrements_positive_values() {
    let v = Atomic64::new(2);
    assert_eq!(v.dec_if_positive(), 1);
    assert_eq!(v.dec_if_positive(), 0);
    // At zero the value is kept and the result is still value minus one.
    assert_eq!(v.dec_if_positive(), -1);
    assert_eq!(v.read(), 0);
    v.set(-3);
    assert_eq!(v.dec_if_positive(), -4);
    assert_eq!(v.read(), -3);
}

#[test]
fn cmpxchg_stores_only_on_match() {
    let v = Atomic64::new(5);
    assert_eq!(v.cmpxchg(4, 9), 5);
    assert_eq!(v.read(), 5);
    assert_eq!(v.cmpxchg(5, 9), 5);
    assert_eq!(v.read(), 9);
}

#[test]
fn xchg_returns_old_value() {
    let v = Atomic64::new(1);
    assert_eq!(v.xchg(42), 1);
    assert_eq!(v.read(), 42);
}

#[test]
fn add_unless_skips_the_forbidden_value() {
    let v = Atomic64::new(3);
    assert_eq!(v.fetch_add_unless(1, 3), 3);
    assert_eq!(v.read(), 3);
    assert!(!v.add_unless(1, 3));
    assert_eq!(v.fetch_add_unless(1, 7), 3);
    assert_eq!(v.read(), 4);
    assert!(v.add_unless(1, 7));
    assert_eq!(v.read(), 5);
}

#[test]
fn inc_not_zero_refuses_zero() {
    let v = Atomic64::new(0);
    assert!(!v.inc_not_zero());
    assert_eq!(v.read(), 0);
    v.set(1);
    assert!(v.inc_not_zero());
    assert_eq!(v.read(), 2);
}

#[test]
fn concurrent_adds_are_not_lost() {
    let v = Arc::new(Atomic64::new(0));
    let handles: Vec<_> = (0..4)
        .map(|_| {
            let v = Arc::clone(&v);
            thread::spawn(move || {
                for _ in 0..10_000 {
                    v.inc();
                }
            })
        })
        .collect();
    for h in handles {
        h.join().unwrap();
    }
    assert_eq!(v.read(), 40_000);
}

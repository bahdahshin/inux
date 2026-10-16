// SPDX-License-Identifier: GPL-2.0-or-later
//! Host tests for `bust_spinlocks`. Linux has no KUnit file for it. The
//! counter is a process-wide static, so these tests share one lock.

use core::sync::atomic::Ordering;
use rinux_lib::bust_spinlocks::{OOPS_IN_PROGRESS, bust_spinlocks};
use std::sync::Mutex;

static LOCK: Mutex<()> = Mutex::new(());

#[test]
fn nested_oopses_wake_klogd_only_at_the_outermost_exit() {
    let _guard = LOCK.lock().unwrap();
    OOPS_IN_PROGRESS.store(0, Ordering::SeqCst);
    let mut unblanks = 0;
    let mut wakes = 0;

    bust_spinlocks(true, || unblanks += 1, || wakes += 1);
    bust_spinlocks(true, || unblanks += 1, || wakes += 1);
    assert_eq!(OOPS_IN_PROGRESS.load(Ordering::SeqCst), 2);

    bust_spinlocks(false, || unblanks += 1, || wakes += 1);
    assert_eq!((unblanks, wakes), (1, 0));
    bust_spinlocks(false, || unblanks += 1, || wakes += 1);
    assert_eq!((unblanks, wakes), (2, 1));
    assert_eq!(OOPS_IN_PROGRESS.load(Ordering::SeqCst), 0);
}

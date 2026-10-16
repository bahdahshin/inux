// SPDX-License-Identifier: GPL-2.0-only
//! Tests for the lock-debugging switch, in the spirit of `lib/debug_locks.c`.

use rinux_lib::debug_locks::{DEBUG_LOCKS, DebugLocks};

#[test]
fn starts_on_and_not_silent() {
    let d = DebugLocks::new();
    assert!(d.enabled());
    assert!(!d.is_silent());
}

#[test]
fn first_off_reports_and_runs_console_verbose() {
    let d = DebugLocks::new();
    let mut called = 0;
    assert!(d.off(|| called += 1));
    assert_eq!(called, 1);
    assert!(!d.enabled());
}

#[test]
fn later_off_calls_are_quiet() {
    let d = DebugLocks::new();
    assert!(d.off(|| {}));
    let mut called = 0;
    assert!(!d.off(|| called += 1));
    assert_eq!(called, 0);
    assert!(!d.enabled());
}

#[test]
fn silent_off_turns_debugging_off_without_console() {
    let d = DebugLocks::new();
    d.set_silent(true);
    let mut called = 0;
    assert!(!d.off(|| called += 1));
    assert_eq!(called, 0);
    assert!(!d.enabled());
}

#[test]
fn off_when_already_off_is_a_no_op() {
    let d = DebugLocks::new();
    d.off(|| {});
    d.set_silent(true);
    assert!(!d.off(|| panic!("console must not run")));
    assert!(!d.enabled());
}

#[test]
fn set_silent_round_trips() {
    let d = DebugLocks::default();
    d.set_silent(true);
    assert!(d.is_silent());
    d.set_silent(false);
    assert!(!d.is_silent());
}

#[test]
fn global_instance_starts_enabled() {
    assert!(DEBUG_LOCKS.enabled());
}

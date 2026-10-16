// SPDX-License-Identifier: GPL-2.0
//! Tests for the rate limiter.

use rinux_lib::ratelimit::RatelimitState;

#[test]
fn burst_then_suppress_then_reset() {
    let mut rs = RatelimitState::new(10, 3);
    // The first `burst` events in a window are allowed.
    assert!(rs.ratelimit(0));
    assert!(rs.ratelimit(0));
    assert!(rs.ratelimit(0));
    // The rest are suppressed and counted.
    assert!(!rs.ratelimit(0));
    assert!(!rs.ratelimit(5));
    assert_eq!(rs.missed(), 2);

    // Once the interval has elapsed the allowance resets (and the suppressed
    // count is cleared, as the kernel reports and resets it).
    assert!(rs.ratelimit(11));
    assert_eq!(rs.missed(), 0);
    assert!(rs.ratelimit(11));
    assert!(rs.ratelimit(11));
    assert!(!rs.ratelimit(11));
    assert_eq!(rs.missed(), 1);
}

#[test]
fn zero_interval_never_limits() {
    let mut rs = RatelimitState::new(0, 10);
    for t in 0..100 {
        assert!(rs.ratelimit(t));
    }
    assert_eq!(rs.missed(), 0);
}

#[test]
fn nonpositive_burst_always_limits() {
    let mut rs = RatelimitState::new(10, 0);
    for t in 0..5 {
        assert!(!rs.ratelimit(t));
    }
    assert_eq!(rs.missed(), 5);
}

#[test]
fn one_per_interval() {
    // burst 1 => at most one event allowed per interval.
    let mut rs = RatelimitState::new(100, 1);
    let mut allowed = 0;
    // 1000 time units, an event every 10: 100 events over 10 windows.
    for t in (0..1000).step_by(10) {
        if rs.ratelimit(t) {
            allowed += 1;
        }
    }
    assert_eq!(allowed, 10, "one event per 100-unit window over 1000 units");
}

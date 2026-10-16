// SPDX-License-Identifier: GPL-2.0-or-later
//! Host tests for `errseq`. Linux has no KUnit file for it, so these cover
//! the sample/set/check protocol the header documents.

use core::sync::atomic::AtomicU32;
use rinux_lib::errseq::{errseq_check, errseq_check_and_advance, errseq_sample, errseq_set};

// Values from include/uapi/asm-generic/errno-base.h.
const EIO: i32 = -5;
const ENOSPC: i32 = -28;

#[test]
fn fresh_value_is_zero_and_clean() {
    let eseq = AtomicU32::new(0);
    assert_eq!(errseq_sample(&eseq), 0);
    assert_eq!(errseq_check(&eseq, 0), 0);
}

#[test]
fn set_error_is_reported_to_a_prior_sample() {
    let eseq = AtomicU32::new(0);
    let since = errseq_sample(&eseq);

    errseq_set(&eseq, EIO);

    assert_eq!(errseq_check(&eseq, since), EIO);
    // `errseq_check` does not advance the sample, so it still reports.
    assert_eq!(errseq_check(&eseq, since), EIO);
}

#[test]
fn invalid_errors_are_ignored() {
    let eseq = AtomicU32::new(0);

    // Zero would clear an error; positive and below -MAX_ERRNO are out of
    // range. The stored value must not change.
    for bad in [0, 1, -4096, i32::MIN] {
        errseq_set(&eseq, bad);
        assert_eq!(eseq.load(core::sync::atomic::Ordering::Relaxed), 0, "{bad}");
    }
}

#[test]
fn set_returns_the_previous_value() {
    let eseq = AtomicU32::new(0);
    assert_eq!(errseq_set(&eseq, EIO), 0);
    // Previous value was zero, so the first set returns the old zero.
    assert_eq!(errseq_set(&eseq, ENOSPC) & 0x0fff, EIO.unsigned_abs());
}

#[test]
fn check_and_advance_reports_once_per_subscriber() {
    let eseq = AtomicU32::new(0);
    let mut since = errseq_sample(&eseq);

    errseq_set(&eseq, EIO);

    assert_eq!(errseq_check_and_advance(&eseq, &mut since), EIO);
    // The error is now seen and `since` has caught up, so no second report.
    assert_eq!(errseq_check_and_advance(&eseq, &mut since), 0);
    assert_eq!(errseq_check(&eseq, since), 0);
}

#[test]
fn unseen_error_still_reaches_a_new_subscriber() {
    let eseq = AtomicU32::new(0);
    errseq_set(&eseq, EIO);

    // Not seen yet, so a fresh sample starts from zero and sees the error.
    let mut since = errseq_sample(&eseq);
    assert_eq!(errseq_check_and_advance(&eseq, &mut since), EIO);
}

#[test]
fn seen_error_is_hidden_from_a_new_subscriber() {
    let eseq = AtomicU32::new(0);
    errseq_set(&eseq, EIO);
    let mut first = errseq_sample(&eseq);
    assert_eq!(errseq_check_and_advance(&eseq, &mut first), EIO);

    // Seen by someone, so a new subscriber does not report the old error.
    let mut late = errseq_sample(&eseq);
    assert_eq!(late, eseq.load(core::sync::atomic::Ordering::Relaxed));
    assert_eq!(errseq_check_and_advance(&eseq, &mut late), 0);
}

#[test]
fn counter_moves_only_after_a_sample() {
    let eseq = AtomicU32::new(0);

    // Two sets with no sample in between: the second does not bump the
    // counter, so the value is just the newest error.
    errseq_set(&eseq, EIO);
    let after_first = eseq.load(core::sync::atomic::Ordering::Relaxed);
    errseq_set(&eseq, ENOSPC);
    let after_second = eseq.load(core::sync::atomic::Ordering::Relaxed);
    assert_eq!(after_first >> 13, after_second >> 13);

    // Sampled, so the next set bumps the counter.
    let mut since = errseq_sample(&eseq);
    assert_eq!(errseq_check_and_advance(&eseq, &mut since), ENOSPC);
    errseq_set(&eseq, EIO);
    let after_third = eseq.load(core::sync::atomic::Ordering::Relaxed);
    assert_eq!(after_third >> 13, (after_second >> 13) + 1);
}

#[test]
fn sampled_subscriber_sees_each_new_error_once() {
    let eseq = AtomicU32::new(0);
    let mut since = errseq_sample(&eseq);

    errseq_set(&eseq, EIO);
    assert_eq!(errseq_check_and_advance(&eseq, &mut since), EIO);

    errseq_set(&eseq, ENOSPC);
    assert_eq!(errseq_check_and_advance(&eseq, &mut since), ENOSPC);
    assert_eq!(errseq_check_and_advance(&eseq, &mut since), 0);
}

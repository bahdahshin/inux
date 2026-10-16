// SPDX-License-Identifier: GPL-2.0-or-later
//! Host tests for `lib/dynamic_queue_limits.c`. Linux has no KUnit file for
//! it, so these cover the starved path, the bounds, avail, the object limit,
//! the reset, and the BUG_ON in `dql_completed()`.

use rinux_lib::dynamic_queue_limits::{DQL_MAX_LIMIT, DQL_MAX_OBJECT, Dql};

#[test]
fn a_new_queue_starts_at_its_minimum_with_nothing_available() {
    let dql = Dql::new(10, 0);
    assert_eq!(dql.limit(), 0);
    assert_eq!(dql.avail(), 0);
    assert_eq!(dql.max_limit, DQL_MAX_LIMIT);
}

#[test]
fn a_starved_queue_raises_its_limit_by_what_it_sent() {
    let mut dql = Dql::new(10, 0);
    // Queue 100 with a limit of 0: over the limit, and nothing is left
    // queued once it completes, so the limit grows by the 100 bytes sent.
    dql.queued(100);
    dql.completed(100, 1);
    assert_eq!(dql.limit(), 100);
}

#[test]
fn the_limit_is_clamped_to_the_maximum() {
    let mut dql = Dql::new(10, 0);
    dql.max_limit = 50;
    dql.queued(100);
    dql.completed(100, 1);
    assert_eq!(dql.limit(), 50);
}

#[test]
fn the_limit_is_clamped_to_the_minimum() {
    let mut dql = Dql::new(10, 0);
    dql.min_limit = 30;
    dql.reset(0);
    assert_eq!(dql.limit(), 30);
}

#[test]
fn avail_is_the_adjusted_limit_less_what_is_queued() {
    let mut dql = Dql::new(10, 0);
    dql.queued(100);
    dql.completed(100, 1);
    // adj_limit = limit (100) + completed (100); 100 queued in total.
    assert_eq!(dql.avail(), 100);
    dql.queued(40);
    assert_eq!(dql.avail(), 60);
}

#[test]
fn a_count_above_the_object_limit_is_ignored() {
    let mut dql = Dql::new(10, 0);
    dql.queued(DQL_MAX_OBJECT + 1);
    assert_eq!(dql.avail(), 0);
}

#[test]
#[should_panic(expected = "more objects completed than queued")]
fn completing_more_than_is_queued_panics() {
    let mut dql = Dql::new(10, 0);
    dql.queued(10);
    dql.completed(11, 1);
}

#[test]
fn reset_clears_the_counters_and_keeps_the_bounds() {
    let mut dql = Dql::new(10, 0);
    dql.max_limit = 50;
    dql.queued(100);
    dql.completed(100, 1);
    dql.reset(5);
    assert_eq!(dql.limit(), 0);
    // As in C, reset leaves adj_limit alone: limit 50 + completed 100.
    assert_eq!(dql.avail(), 150);
    assert_eq!(dql.max_limit, 50);
}

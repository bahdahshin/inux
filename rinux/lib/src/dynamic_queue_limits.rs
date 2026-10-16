// SPDX-License-Identifier: GPL-2.0
//! Dynamic queue limits: track how much is queued against how much has
//! completed, and adjust a byte limit so a queue neither starves nor
//! over-buffers.
//!
//! Ported from `lib/dynamic_queue_limits.c` and the inline helpers in
//! `include/linux/dynamic_queue_limits.h`. Time is passed in as `now`, in the
//! same units as the C `jiffies`, so the caller owns the clock.
//!
//! Not ported yet: the stall detector (`dql_check_stall()` and
//! `dql_queue_stall()`). It keeps a bitmap of queue times and runs only when
//! `stall_thrs` is set. This port keeps `stall_thrs` at 0, which is the state
//! `dql_init()` leaves it in, so the stall path never runs here.

/// `DQL_MAX_OBJECT`: the most objects one `queued()` call may record.
pub const DQL_MAX_OBJECT: u32 = u32::MAX / 16;
/// `DQL_MAX_LIMIT`: the largest limit a queue can be given.
pub const DQL_MAX_LIMIT: u32 = (u32::MAX / 2) - DQL_MAX_OBJECT;

/// `POSDIFF(a, b)`: `a - b` when it is positive as a signed value, else 0.
fn posdiff(a: u32, b: u32) -> u32 {
    let d = a.wrapping_sub(b);
    if (d as i32) > 0 { d } else { 0 }
}

/// `AFTER_EQ(a, b)`: `a >= b` in wrapping arithmetic.
fn after_eq(a: u32, b: u32) -> bool {
    (a.wrapping_sub(b) as i32) >= 0
}

/// `time_after()`: `a` is later than `b` in wrapping time.
fn time_after(a: u64, b: u64) -> bool {
    (b.wrapping_sub(a) as i64) < 0
}

/// `struct dql`: the state of one queue.
#[derive(Debug, Clone)]
pub struct Dql {
    /// Current limit, in objects (bytes for a network queue).
    limit: u32,
    /// `limit + num_completed`: the value `avail()` measures against.
    adj_limit: u32,
    /// Total ever queued.
    num_queued: u32,
    /// Count from the last `queued()` call.
    last_obj_cnt: u32,
    /// Total ever completed.
    num_completed: u32,
    /// Over-limit in the previous interval.
    prev_ovlimit: u32,
    /// Queued total at the previous completion.
    prev_num_queued: u32,
    /// Count from the queuing before the last one.
    prev_last_obj_cnt: u32,
    /// Lowest slack seen in the current window.
    lowest_slack: u32,
    /// When the current slack window started.
    slack_start_time: u64,
    /// Upper bound on the limit.
    pub max_limit: u32,
    /// Lower bound on the limit.
    pub min_limit: u32,
    /// Length of the slack window.
    pub slack_hold_time: u32,
}

impl Dql {
    /// `dql_init()`: a queue with the default bounds and a slack window of
    /// `hold_time`, starting at `now`.
    pub fn new(hold_time: u32, now: u64) -> Dql {
        let mut dql = Dql {
            limit: 0,
            adj_limit: 0,
            num_queued: 0,
            last_obj_cnt: 0,
            num_completed: 0,
            prev_ovlimit: 0,
            prev_num_queued: 0,
            prev_last_obj_cnt: 0,
            lowest_slack: u32::MAX,
            slack_start_time: now,
            max_limit: DQL_MAX_LIMIT,
            min_limit: 0,
            slack_hold_time: hold_time,
        };
        dql.reset(now);
        dql
    }

    /// `dql_reset()`: restore the dynamic state to its initial values. The
    /// bounds and the hold time are kept.
    pub fn reset(&mut self, now: u64) {
        self.limit = self.min_limit;
        self.num_queued = 0;
        self.num_completed = 0;
        self.last_obj_cnt = 0;
        self.prev_num_queued = 0;
        self.prev_last_obj_cnt = 0;
        self.prev_ovlimit = 0;
        self.lowest_slack = u32::MAX;
        self.slack_start_time = now;
    }

    /// The current limit.
    pub fn limit(&self) -> u32 {
        self.limit
    }

    /// `dql_queued()`: record `count` objects queued. The caller checks
    /// [`Dql::avail`] first. A count above [`DQL_MAX_OBJECT`] is ignored.
    pub fn queued(&mut self, count: u32) {
        if count > DQL_MAX_OBJECT {
            return;
        }
        self.last_obj_cnt = count;
        self.num_queued = self.num_queued.wrapping_add(count);
    }

    /// `dql_avail()`: how many objects may still be queued. Negative when the
    /// queue is over its limit.
    pub fn avail(&self) -> i32 {
        self.adj_limit.wrapping_sub(self.num_queued) as i32
    }

    /// `dql_completed()`: record `count` completed objects and recompute the
    /// limit. `now` is the current time.
    ///
    /// Panics when `count` is more than is queued and not yet completed, as
    /// the C `BUG_ON()` does.
    pub fn completed(&mut self, count: u32, now: u64) {
        let num_queued = self.num_queued;
        assert!(
            count <= num_queued.wrapping_sub(self.num_completed),
            "dql_completed: more objects completed than queued"
        );

        let completed = self.num_completed.wrapping_add(count);
        let mut limit = self.limit;
        let mut ovlimit = posdiff(num_queued.wrapping_sub(self.num_completed), limit);
        let inprogress = num_queued.wrapping_sub(completed);
        let prev_inprogress = self.prev_num_queued.wrapping_sub(self.num_completed);
        let all_prev_completed = after_eq(completed, self.prev_num_queued);

        if (ovlimit != 0 && inprogress == 0) || (self.prev_ovlimit != 0 && all_prev_completed) {
            // Starved: the queue ran dry while over its limit, so raise it by
            // what was sent and completed, plus the previous over-limit.
            limit = limit
                .wrapping_add(posdiff(completed, self.prev_num_queued))
                .wrapping_add(self.prev_ovlimit);
            self.slack_start_time = now;
            self.lowest_slack = u32::MAX;
        } else if inprogress != 0 && prev_inprogress != 0 && !all_prev_completed {
            // Busy the whole interval: track the slack and lower the limit
            // once it has been seen for a full hold window.
            let slack = posdiff(
                limit.wrapping_add(self.prev_ovlimit),
                2u32.wrapping_mul(completed.wrapping_sub(self.num_completed)),
            );
            let slack_last_objs = if self.prev_ovlimit != 0 {
                posdiff(self.prev_last_obj_cnt, self.prev_ovlimit)
            } else {
                0
            };
            let slack = slack.max(slack_last_objs);

            if slack < self.lowest_slack {
                self.lowest_slack = slack;
            }

            if time_after(
                now,
                self.slack_start_time
                    .wrapping_add(u64::from(self.slack_hold_time)),
            ) {
                limit = posdiff(limit, self.lowest_slack);
                self.slack_start_time = now;
                self.lowest_slack = u32::MAX;
            }
        }

        // Enforce the bounds.
        limit = limit.clamp(self.min_limit, self.max_limit);

        if limit != self.limit {
            self.limit = limit;
            ovlimit = 0;
        }

        self.adj_limit = limit.wrapping_add(completed);
        self.prev_ovlimit = ovlimit;
        self.prev_last_obj_cnt = self.last_obj_cnt;
        self.num_completed = completed;
        self.prev_num_queued = num_queued;
    }
}

// SPDX-License-Identifier: GPL-2.0-only
//! Utilization clamp buckets: the value-to-bucket mapping that keeps per-CPU
//! bookkeeping of clamp requests small.
//!
//! Ported from `kernel/sched/sched.h`: `UCLAMP_BUCKET_DELTA` and
//! `uclamp_bucket_id()`, with `SCHED_CAPACITY_SCALE` from `include/linux/sched.h`
//! and the default `UCLAMP_BUCKETS_COUNT` of 5 from `init/Kconfig`. A clamp
//! value in `0..=1024` falls in one of the buckets; the top bucket also takes
//! everything above the last boundary.

/// `SCHED_CAPACITY_SCALE`: full capacity, `1 << SCHED_CAPACITY_SHIFT`.
pub const SCHED_CAPACITY_SCALE: u32 = 1 << 10;
/// `UCLAMP_BUCKETS`: the configured bucket count (the Kconfig default).
pub const UCLAMP_BUCKETS: u32 = 5;
/// `UCLAMP_BUCKET_DELTA`: the width of one bucket, rounded to nearest.
pub const UCLAMP_BUCKET_DELTA: u32 = div_round_closest(SCHED_CAPACITY_SCALE, UCLAMP_BUCKETS);

/// `DIV_ROUND_CLOSEST()` for unsigned operands.
pub const fn div_round_closest(n: u32, d: u32) -> u32 {
    (n + d / 2) / d
}

/// `uclamp_bucket_id()`: the bucket a clamp value belongs to.
pub const fn uclamp_bucket_id(clamp_value: u32) -> u32 {
    let id = clamp_value / UCLAMP_BUCKET_DELTA;
    if id < UCLAMP_BUCKETS - 1 {
        id
    } else {
        UCLAMP_BUCKETS - 1
    }
}

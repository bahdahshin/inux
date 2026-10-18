// SPDX-License-Identifier: GPL-2.0
//! The timer wheel's bucket arithmetic: which bucket a timer goes in, and
//! the expiry that bucket represents.
//!
//! Ported from `kernel/time/timer.c`: the level constants (`LVL_*`,
//! `WHEEL_*`), `calc_index()` and `calc_wheel_index()`. A timer lands in the
//! level whose granularity covers its delta, and the bucket is rounded up to
//! that granularity so a timer never fires early. The wheel's queues, the
//! per-CPU bases and the locking are not here; `calc_wheel_index()` only says
//! where a timer goes.

/// `LVL_CLK_SHIFT`: each level is 8 times coarser than the one below it.
pub const LVL_CLK_SHIFT: u32 = 3;
/// `LVL_BITS`: each level has 64 buckets.
pub const LVL_BITS: u32 = 6;
/// `LVL_SIZE`.
pub const LVL_SIZE: u64 = 1 << LVL_BITS;
/// `LVL_MASK`.
pub const LVL_MASK: u64 = LVL_SIZE - 1;
/// `LVL_DEPTH`: the number of levels. `timer.c` uses 9 when `HZ > 100`, and
/// rinux runs at `HZ` 1000.
pub const LVL_DEPTH: u32 = 9;
/// `WHEEL_SIZE`: the buckets across all levels.
pub const WHEEL_SIZE: u64 = LVL_SIZE * LVL_DEPTH as u64;

/// `LVL_SHIFT(n)`: the shift of level `n`'s granularity.
pub const fn lvl_shift(n: u32) -> u32 {
    n * LVL_CLK_SHIFT
}

/// `LVL_GRAN(n)`: the granularity of level `n`, in ticks.
pub const fn lvl_gran(n: u32) -> u64 {
    1 << lvl_shift(n)
}

/// `LVL_START(n)`: the delta from which level `n` is used.
pub const fn lvl_start(n: u32) -> u64 {
    (LVL_SIZE - 1) << ((n - 1) * LVL_CLK_SHIFT)
}

/// `LVL_OFFS(n)`: the index of level `n`'s first bucket.
pub const fn lvl_offs(n: u32) -> u64 {
    n as u64 * LVL_SIZE
}

/// `WHEEL_TIMEOUT_CUTOFF`: deltas at or beyond this are clamped.
pub const WHEEL_TIMEOUT_CUTOFF: u64 = lvl_start(LVL_DEPTH);
/// `WHEEL_TIMEOUT_MAX`: the expiry an obscene timeout is forced to.
pub const WHEEL_TIMEOUT_MAX: u64 = WHEEL_TIMEOUT_CUTOFF - lvl_gran(LVL_DEPTH - 1);

/// `calc_index()`: the bucket on level `lvl` for `expires`, and the expiry
/// that bucket stands for. The expiry is rounded up to the level granularity,
/// so the bucket never fires before `expires`.
pub fn calc_index(expires: u64, lvl: u32) -> (u64, u64) {
    let rounded = (expires >> lvl_shift(lvl)) + 1;
    let bucket_expiry = rounded << lvl_shift(lvl);
    (lvl_offs(lvl) + (rounded & LVL_MASK), bucket_expiry)
}

/// `calc_wheel_index()`: the bucket for a timer that expires at `expires`
/// when the clock reads `clk`. Returns the bucket index and its expiry.
pub fn calc_wheel_index(expires: u64, clk: u64) -> (u64, u64) {
    let delta = expires.wrapping_sub(clk);
    for lvl in 1..LVL_DEPTH {
        if delta < lvl_start(lvl) {
            return calc_index(expires, lvl - 1);
        }
    }
    if (delta as i64) < 0 {
        // The timer is already due: it goes in the bucket for the current tick.
        return (clk & LVL_MASK, clk);
    }
    // Obscene timeouts are forced to the wheel's capacity limit.
    let expires = if delta >= WHEEL_TIMEOUT_CUTOFF {
        clk + WHEEL_TIMEOUT_MAX
    } else {
        expires
    };
    calc_index(expires, LVL_DEPTH - 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn level_constants() {
        assert_eq!(WHEEL_SIZE, 576);
        assert_eq!(lvl_gran(1), 8);
        assert_eq!(lvl_start(1), 63);
        assert_eq!(lvl_start(2), 504);
        assert_eq!(lvl_offs(2), 128);
    }

    #[test]
    fn a_short_timer_goes_in_level_zero_and_rounds_up() {
        // expires 20 at clk 10: level 0, granularity 1, rounded to 21.
        assert_eq!(calc_wheel_index(20, 10), (21 & LVL_MASK, 21));
    }

    #[test]
    fn a_timer_in_the_outer_levels_rounds_to_its_granularity() {
        // delta 1000 is past START(2) = 504, so level 2 (granularity 64).
        let (idx, bucket) = calc_wheel_index(1000, 0);
        assert!(idx >= lvl_offs(2) && idx < lvl_offs(3));
        assert_eq!(bucket % lvl_gran(2), 0);
        assert!(bucket > 1000);
    }

    #[test]
    fn a_bucket_never_fires_early() {
        for clk in [0u64, 1, 7, 8, 1000, 123_456] {
            for delta in 0..5000u64 {
                let expires = clk + delta;
                let (idx, bucket) = calc_wheel_index(expires, clk);
                assert!(idx < WHEEL_SIZE, "idx {idx} for delta {delta}");
                assert!(bucket > expires, "bucket {bucket} for expires {expires}");
            }
        }
    }

    #[test]
    fn a_due_timer_goes_in_the_current_tick_bucket() {
        assert_eq!(calc_wheel_index(5, 10), (10 & LVL_MASK, 10));
    }

    #[test]
    fn an_obscene_timeout_is_clamped_to_the_wheel_capacity() {
        let clk = 1000;
        let (idx, bucket) = calc_wheel_index(clk + WHEEL_TIMEOUT_CUTOFF + 1_000_000, clk);
        assert!(idx < WHEEL_SIZE);
        assert!(bucket <= clk + WHEEL_TIMEOUT_MAX + lvl_gran(LVL_DEPTH - 1));
    }
}

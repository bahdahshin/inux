// SPDX-License-Identifier: GPL-2.0
//! Time namespace offsets: the per-namespace shift of the monotonic and boot
//! clocks, and how a time in a namespace maps to the host clock.
//!
//! Ported from `kernel/time/namespace.c`, `do_timens_ktime_to_host()`, and the
//! offset arithmetic of `include/linux/time_namespace.h` (`timens_add_monotonic()`
//! and `timens_add_boottime()`). The namespace objects, their refcounts, the
//! `/proc` interface and the vDSO page are not ported here.

/// `NSEC_PER_SEC`.
pub const NSEC_PER_SEC: i64 = 1_000_000_000;

/// `CLOCK_MONOTONIC`.
pub const CLOCK_MONOTONIC: i32 = 1;
/// `CLOCK_BOOTTIME`.
pub const CLOCK_BOOTTIME: i32 = 7;
/// `CLOCK_BOOTTIME_ALARM`.
pub const CLOCK_BOOTTIME_ALARM: i32 = 9;

/// `struct timespec64`: seconds and nanoseconds, with nanoseconds in
/// `0..NSEC_PER_SEC` once normalized.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Timespec64 {
    /// Seconds.
    pub tv_sec: i64,
    /// Nanoseconds.
    pub tv_nsec: i64,
}

/// `timespec64_add()`: the sum, with the nanoseconds carried into the seconds.
pub fn timespec64_add(a: Timespec64, b: Timespec64) -> Timespec64 {
    let mut sec = a.tv_sec + b.tv_sec;
    let mut nsec = a.tv_nsec + b.tv_nsec;
    if nsec >= NSEC_PER_SEC {
        nsec -= NSEC_PER_SEC;
        sec += 1;
    }
    Timespec64 {
        tv_sec: sec,
        tv_nsec: nsec,
    }
}

/// `timespec64_to_ktime()`: nanoseconds, saturated to the `ktime_t` range.
pub fn timespec64_to_ktime(ts: Timespec64) -> i64 {
    ts.tv_sec
        .saturating_mul(NSEC_PER_SEC)
        .saturating_add(ts.tv_nsec)
}

/// `struct timens_offsets`: how far a namespace's clocks are shifted from
/// the host's.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TimensOffsets {
    /// The `CLOCK_MONOTONIC` offset.
    pub monotonic: Timespec64,
    /// The `CLOCK_BOOTTIME` offset.
    pub boottime: Timespec64,
}

/// `timens_add_monotonic()`: a host monotonic time as the namespace sees it.
pub fn timens_add_monotonic(ts: Timespec64, offsets: &TimensOffsets) -> Timespec64 {
    timespec64_add(ts, offsets.monotonic)
}

/// `timens_add_boottime()`: a host boot time as the namespace sees it.
pub fn timens_add_boottime(ts: Timespec64, offsets: &TimensOffsets) -> Timespec64 {
    timespec64_add(ts, offsets.boottime)
}

/// `do_timens_ktime_to_host()`: a time given in a namespace, converted to the
/// host clock for the monotonic and boot clocks. A time before the offset has
/// already expired and becomes 0; other clocks are unchanged.
pub fn do_timens_ktime_to_host(clockid: i32, tim: i64, offsets: &TimensOffsets) -> i64 {
    let offset = match clockid {
        CLOCK_MONOTONIC => timespec64_to_ktime(offsets.monotonic),
        CLOCK_BOOTTIME | CLOCK_BOOTTIME_ALARM => timespec64_to_ktime(offsets.boottime),
        _ => return tim,
    };
    if tim < offset {
        // An absolute time before the namespace's start has already passed.
        0
    } else {
        // `tim - offset` cannot overflow once `tim >= offset` and `offset >= 0`;
        // for a negative offset it is clamped to KTIME_MAX, as C does.
        tim.checked_sub(offset).unwrap_or(i64::MAX)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ts(sec: i64, nsec: i64) -> Timespec64 {
        Timespec64 {
            tv_sec: sec,
            tv_nsec: nsec,
        }
    }

    #[test]
    fn timespec_add_carries_nanoseconds() {
        assert_eq!(
            timespec64_add(ts(1, 900_000_000), ts(2, 200_000_000)),
            ts(4, 100_000_000)
        );
    }

    #[test]
    fn monotonic_time_is_shifted_by_the_offset() {
        let off = TimensOffsets {
            monotonic: ts(100, 0),
            boottime: ts(0, 0),
        };
        assert_eq!(timens_add_monotonic(ts(5, 0), &off), ts(105, 0));
        assert_eq!(timens_add_boottime(ts(5, 0), &off), ts(5, 0));
    }

    #[test]
    fn host_time_is_the_namespace_time_minus_the_offset() {
        let off = TimensOffsets {
            monotonic: ts(10, 0),
            boottime: ts(20, 0),
        };
        assert_eq!(
            do_timens_ktime_to_host(CLOCK_MONOTONIC, 15 * NSEC_PER_SEC, &off),
            5 * NSEC_PER_SEC
        );
        assert_eq!(
            do_timens_ktime_to_host(CLOCK_BOOTTIME, 25 * NSEC_PER_SEC, &off),
            5 * NSEC_PER_SEC
        );
    }

    #[test]
    fn an_already_passed_absolute_time_becomes_zero() {
        let off = TimensOffsets {
            monotonic: ts(10, 0),
            boottime: ts(0, 0),
        };
        assert_eq!(
            do_timens_ktime_to_host(CLOCK_MONOTONIC, 5 * NSEC_PER_SEC, &off),
            0
        );
    }

    #[test]
    fn other_clocks_are_not_shifted() {
        let off = TimensOffsets {
            monotonic: ts(10, 0),
            boottime: ts(10, 0),
        };
        assert_eq!(do_timens_ktime_to_host(0, 42, &off), 42);
    }
}

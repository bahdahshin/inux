// SPDX-License-Identifier: GPL-2.0-only
//! Scalable reference count for RCU-managed objects.
//!
//! Ported from `lib/rcuref.c` and `include/linux/rcuref.h`.
//!
//! The counter is unsigned and offset by one: `ONEREF` (0) means one
//! reference, and a value at or above `RELEASED` is the dead zone. `get()`
//! and `put()` change the counter unconditionally and check the result, so
//! the fast path is one atomic operation. The slow paths handle the
//! saturation and dead zones.
//!
//! Deviations from C:
//! - `rcuref_put()` disables preemption around `__rcuref_put()`. Rust has no
//!   preemption control here, so [`RcuRef::put`] and [`RcuRef::put_rcusafe`]
//!   are the same operation. The RCU lockdep check is not reproduced.
//! - The `WARN_ONCE()` calls become no-ops. The saturated and imbalanced
//!   states are still set exactly as in C, and callers can see them through
//!   [`RcuRef::read`] and [`RcuRef::is_dead`].

use core::sync::atomic::{AtomicU32, Ordering, fence};

/// `RCUREF_ONEREF`: one reference is held.
pub const RCUREF_ONEREF: u32 = 0x0000_0000;
/// `RCUREF_MAXREF`: the largest count before saturation.
pub const RCUREF_MAXREF: u32 = 0x7FFF_FFFF;
/// `RCUREF_SATURATED`: the saturation marker.
pub const RCUREF_SATURATED: u32 = 0xA000_0000;
/// `RCUREF_RELEASED`: the start of the dead zone.
pub const RCUREF_RELEASED: u32 = 0xC000_0000;
/// `RCUREF_DEAD`: the object is dead.
pub const RCUREF_DEAD: u32 = 0xE000_0000;
/// `RCUREF_NOREF`: the last reference was dropped.
pub const RCUREF_NOREF: u32 = 0xFFFF_FFFF;

/// `rcuref_t`: a reference count with the RCU-safe get and put rules.
#[derive(Debug)]
pub struct RcuRef {
    refcnt: AtomicU32,
}

impl RcuRef {
    /// `rcuref_init()`: starts with `cnt` references.
    pub const fn new(cnt: u32) -> Self {
        Self {
            refcnt: AtomicU32::new(cnt.wrapping_sub(1)),
        }
    }

    /// `rcuref_read()`: the number of references, or 0 in the dead zone.
    pub fn read(&self) -> u32 {
        let c = self.refcnt.load(Ordering::Relaxed);
        if c >= RCUREF_RELEASED { 0 } else { c + 1 }
    }

    /// `rcuref_is_dead()`: true in the dead zone, excluding `NOREF`.
    pub fn is_dead(&self) -> bool {
        let c = self.refcnt.load(Ordering::Relaxed);
        (RCUREF_RELEASED..RCUREF_NOREF).contains(&c)
    }

    /// `rcuref_get()`: takes a reference. Fails once the object is dead.
    #[must_use]
    pub fn get(&self) -> bool {
        // The increment is relaxed: the lookup that found the object orders it.
        let old = self.refcnt.fetch_add(1, Ordering::Relaxed);
        if (old.wrapping_add(1) as i32) >= 0 {
            return true;
        }
        self.get_slowpath()
    }

    /// `rcuref_get_slowpath()`: the get() result when the count is outside
    /// the valid zone.
    fn get_slowpath(&self) -> bool {
        let cnt = self.refcnt.load(Ordering::Relaxed);

        // Already dead: undo the increment, staying inside the dead zone.
        if cnt >= RCUREF_RELEASED {
            self.refcnt.store(RCUREF_DEAD, Ordering::Relaxed);
            return false;
        }

        // Saturated: keep the count at the saturation marker so it cannot wrap.
        if cnt > RCUREF_MAXREF {
            self.refcnt.store(RCUREF_SATURATED, Ordering::Relaxed);
        }
        true
    }

    /// `rcuref_put()` without the preemption guard. See the module docs.
    ///
    /// Returns true for the last reference, when the caller may destroy the
    /// object.
    #[must_use]
    pub fn put(&self) -> bool {
        self.put_rcusafe()
    }

    /// `rcuref_put_rcusafe()`: the put() for callers already in an RCU
    /// read-side section. Returns true for the last reference.
    #[must_use]
    pub fn put_rcusafe(&self) -> bool {
        // Release ordering: prior loads and stores happen before the drop.
        let cnt = self.refcnt.fetch_sub(1, Ordering::Release).wrapping_sub(1);
        if (cnt as i32) >= 0 {
            return false;
        }
        self.put_slowpath(cnt)
    }

    /// `rcuref_put_slowpath()`: the put() result when the count is outside
    /// the valid zone. `cnt` is the value after the decrement.
    fn put_slowpath(&self, cnt: u32) -> bool {
        // This drop was the last reference. Mark the object dead, unless a
        // concurrent get() raised the count again or a put() already marked it.
        if cnt == RCUREF_NOREF {
            if self
                .refcnt
                .compare_exchange(
                    RCUREF_NOREF,
                    RCUREF_DEAD,
                    Ordering::Release,
                    Ordering::Relaxed,
                )
                .is_err()
            {
                return false;
            }
            // Acquire ordering for the caller, which will destroy the object.
            fence(Ordering::Acquire);
            return true;
        }

        // Imbalanced put(): the count was already in the dead zone.
        if cnt >= RCUREF_RELEASED {
            self.refcnt.store(RCUREF_DEAD, Ordering::Relaxed);
            return false;
        }

        // A put() on a saturated count: restore the saturation marker.
        if cnt > RCUREF_MAXREF {
            self.refcnt.store(RCUREF_SATURATED, Ordering::Relaxed);
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_after_death_fails_and_keeps_the_count_dead() {
        let r = RcuRef::new(1);
        assert!(r.put());
        assert!(!r.get());
        assert_eq!(r.read(), 0);
        assert!(r.is_dead());
        // The failed get() leaves the counter at DEAD, as the C comment says.
        assert_eq!(r.refcnt.load(Ordering::Relaxed), RCUREF_DEAD);
    }

    #[test]
    fn saturation_is_marked_and_stays_in_the_saturation_zone() {
        let r = RcuRef::new(1);
        r.refcnt.store(RCUREF_MAXREF, Ordering::Relaxed);
        assert!(r.get(), "a get() past MAXREF still succeeds");
        assert_eq!(r.refcnt.load(Ordering::Relaxed), RCUREF_SATURATED);
        assert!(!r.is_dead());
        assert!(!r.put(), "a put() on a saturated count never destroys it");
        assert_eq!(r.refcnt.load(Ordering::Relaxed), RCUREF_SATURATED);
    }

    #[test]
    fn imbalanced_put_in_the_dead_zone_restores_dead() {
        let r = RcuRef::new(1);
        r.refcnt.store(RCUREF_DEAD, Ordering::Relaxed);
        assert!(!r.put());
        assert_eq!(r.refcnt.load(Ordering::Relaxed), RCUREF_DEAD);
    }

    #[test]
    fn last_put_moves_noref_to_dead_only_once() {
        // ONEREF minus one is NOREF, so this put takes the slow path and wins.
        let r = RcuRef::new(1);
        assert!(r.put_rcusafe());
        assert_eq!(r.refcnt.load(Ordering::Relaxed), RCUREF_DEAD);
        assert!(!r.put_slowpath(RCUREF_NOREF));
    }
}

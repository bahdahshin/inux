// SPDX-License-Identifier: GPL-2.0
//! Clock event devices: converting a time to device ticks, and programming
//! the next interrupt, with the minimum-delta retry.
//!
//! Ported from `kernel/time/clockevents.c`: `cev_delta2ns()` and
//! `clockevent_delta2ns()`, `clockevents_increase_min_delta()`,
//! `clockevents_program_min_delta()` (the `GENERIC_CLOCKEVENTS_MIN_ADJUST`
//! variant that x86 selects), and `clockevents_program_event()` for a device
//! that is not coupled to a clocksource. The hardware is the
//! [`SetNextEvent`] trait. The device registry, the state machine beyond the
//! shutdown flag, the sysfs files and the coupled-device paths are not ported.

/// `NSEC_PER_SEC`.
pub const NSEC_PER_SEC: u64 = 1_000_000_000;
/// `HZ` for rinux.
pub const HZ: u64 = 1000;
/// `MIN_DELTA_LIMIT`: the largest minimum delta the retry may grow to.
pub const MIN_DELTA_LIMIT: u64 = NSEC_PER_SEC / HZ;

/// The hardware that accepts the next event. A return of 0 means the device
/// took the value; anything else means it could not, and the caller retries.
pub trait SetNextEvent {
    /// `set_next_event()`: fire `cycles` device ticks from now.
    fn set_next_event(&mut self, cycles: u64) -> i32;
}

/// The fields of `struct clock_event_device` that this module reads and
/// writes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClockEventDevice {
    /// `mult`: the nanoseconds-to-ticks scale numerator.
    pub mult: u64,
    /// `shift`: the scale shift.
    pub shift: u32,
    /// `min_delta_ns`: the shortest delta the device can be programmed with.
    pub min_delta_ns: u64,
    /// `max_delta_ns`: the longest delta one event can cover.
    pub max_delta_ns: u64,
    /// `min_delta_ticks`: `min_delta_ns` in device ticks.
    pub min_delta_ticks: u64,
    /// `next_event`: the expiry the device is programmed for, in ns.
    pub next_event: i64,
    /// `next_event_forced`: the device was programmed with its minimum delta.
    pub next_event_forced: bool,
    /// `retries`: how many times a `set_next_event()` was attempted.
    pub retries: u64,
    /// The device is shut down, so events are not programmed.
    pub shutdown: bool,
}

/// `-ETIME`: the device could not be programmed in time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Etime;

/// `cev_delta2ns()`: convert `latch` device ticks to nanoseconds. The upper
/// bound (`ismax`) omits the rounding add when the scale is above 1 GHz, as C
/// does. Deltas under a microsecond are raised to a microsecond.
pub fn cev_delta2ns(latch: u64, mult: u64, shift: u32, ismax: bool) -> u64 {
    let mult = if mult == 0 { 1 } else { mult };
    let mut clc = latch << shift;
    let rnd = mult - 1;

    // If converting back does not give `latch`, the shift overflowed.
    if (clc >> shift) != latch {
        clc = u64::MAX;
    }

    // Round up so the nanoseconds convert back to at least `latch` ticks,
    // unless that would overflow or it is a maximum on a fast device.
    if (u64::MAX - clc > rnd) && (!ismax || mult <= (1u64 << shift)) {
        clc += rnd;
    }

    clc /= mult;

    // Deltas less than one microsecond are noise.
    clc.max(1000)
}

/// `clockevent_delta2ns()`: [`cev_delta2ns`] for a minimum or regular delta.
pub fn clockevent_delta2ns(latch: u64, dev: &mut ClockEventDevice) -> u64 {
    if dev.mult == 0 {
        dev.mult = 1;
    }
    cev_delta2ns(latch, dev.mult, dev.shift, false)
}

impl ClockEventDevice {
    /// `clockevents_increase_min_delta()`: grow the minimum delta after the
    /// device failed to take it, up to [`MIN_DELTA_LIMIT`]. Gives up with
    /// `-ETIME` once the limit is reached.
    fn increase_min_delta(&mut self) -> Result<(), Etime> {
        if self.min_delta_ns >= MIN_DELTA_LIMIT {
            self.next_event = i64::MAX;
            return Err(Etime);
        }
        if self.min_delta_ns < 5000 {
            self.min_delta_ns = 5000;
        } else {
            self.min_delta_ns += self.min_delta_ns >> 1;
        }
        if self.min_delta_ns > MIN_DELTA_LIMIT {
            self.min_delta_ns = MIN_DELTA_LIMIT;
        }
        Ok(())
    }

    /// `clockevents_program_min_delta()`: program the device for its minimum
    /// delta. Three refusals in a row grow the minimum delta; if that cannot
    /// grow any further the call gives up.
    pub fn program_min_delta<H: SetNextEvent>(
        &mut self,
        hw: &mut H,
        now: i64,
    ) -> Result<(), Etime> {
        let mut i = 0;
        loop {
            let delta = self.min_delta_ns;
            self.next_event = now.saturating_add(delta as i64);

            if self.shutdown {
                return Ok(());
            }

            self.retries += 1;
            let clc = ((delta as u128 * self.mult as u128) >> self.shift) as u64;
            if hw.set_next_event(clc) == 0 {
                return Ok(());
            }

            i += 1;
            if i > 2 {
                // Three refusals: try a larger minimum delta, then start over.
                self.increase_min_delta()?;
                i = 0;
            }
        }
    }

    /// The programmed event has fired. A forced (minimum-delta) event is
    /// pending only until it fires, so the interrupt handler clears the flag
    /// before it reprograms. Without this, the next program would see a stale
    /// forced flag and return without arming anything.
    pub fn event_fired(&mut self) {
        self.next_event_forced = false;
    }

    /// `clockevents_program_event()` for a device not coupled to a
    /// clocksource. Programs the device to fire at `expires` (ns). `force`
    /// allows the minimum delta when `expires` cannot be reached.
    pub fn program_event<H: SetNextEvent>(
        &mut self,
        hw: &mut H,
        expires: i64,
        now: i64,
        force: bool,
    ) -> Result<(), Etime> {
        self.next_event = expires;
        if self.shutdown {
            return Ok(());
        }

        let mut delta = expires - now;
        // Required for the periodic tick during early boot.
        if delta <= 0 && !force {
            return Err(Etime);
        }

        if delta > self.min_delta_ns as i64 {
            delta = delta.min(self.max_delta_ns as i64);
            let cycles = ((delta as u128 * self.mult as u128) >> self.shift) as u64;
            if hw.set_next_event(cycles) == 0 {
                self.next_event_forced = false;
                return Ok(());
            }
        }

        if self.next_event_forced {
            return Ok(());
        }

        if hw.set_next_event(self.min_delta_ticks) != 0
            && (!force || self.program_min_delta(hw, now).is_err())
        {
            return Err(Etime);
        }
        self.next_event_forced = true;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A device at 1 tick per ns (mult 1 << 10, shift 10).
    fn dev() -> ClockEventDevice {
        ClockEventDevice {
            mult: 1 << 10,
            shift: 10,
            min_delta_ns: 10_000,
            max_delta_ns: 1_000_000_000,
            min_delta_ticks: 10_000,
            next_event: 0,
            next_event_forced: false,
            retries: 0,
            shutdown: false,
        }
    }

    /// Records each programmed value; refuses the first `refuse` attempts.
    struct Hw {
        refuse: u32,
        calls: [u64; 16],
        n: usize,
    }

    impl Hw {
        fn new(refuse: u32) -> Self {
            Self {
                refuse,
                calls: [0; 16],
                n: 0,
            }
        }

        fn seen(&self) -> &[u64] {
            &self.calls[..self.n]
        }
    }

    impl SetNextEvent for Hw {
        fn set_next_event(&mut self, cycles: u64) -> i32 {
            self.calls[self.n] = cycles;
            self.n += 1;
            if self.refuse > 0 {
                self.refuse -= 1;
                return -1;
            }
            0
        }
    }

    #[test]
    fn delta2ns_is_the_latch_scaled_back() {
        // 1000 ticks at 1 tick/ns is 1000 ns.
        assert_eq!(cev_delta2ns(1000, 1 << 10, 10, false), 1000);
    }

    #[test]
    fn delta2ns_raises_tiny_deltas_to_a_microsecond() {
        assert_eq!(cev_delta2ns(10, 1 << 10, 10, false), 1000);
    }

    #[test]
    fn delta2ns_saturates_on_overflow() {
        assert_eq!(cev_delta2ns(u64::MAX, 1 << 10, 10, false), u64::MAX >> 10);
    }

    #[test]
    fn a_future_expiry_is_programmed_as_a_cycle_count() {
        let mut d = dev();
        let mut hw = Hw::new(0);
        assert_eq!(d.program_event(&mut hw, 50_000, 0, false), Ok(()));
        assert_eq!(hw.seen(), &[50_000]);
        assert_eq!(d.next_event, 50_000);
        assert!(!d.next_event_forced);
    }

    #[test]
    fn a_past_expiry_is_refused_unless_forced() {
        let mut d = dev();
        let mut hw = Hw::new(0);
        assert_eq!(d.program_event(&mut hw, 100, 200, false), Err(Etime));
        assert!(hw.seen().is_empty());
    }

    #[test]
    fn a_delta_below_the_minimum_uses_the_minimum_ticks() {
        let mut d = dev();
        let mut hw = Hw::new(0);
        assert_eq!(d.program_event(&mut hw, 5_000, 0, false), Ok(()));
        // 5000 ns is under min_delta, so the device gets its minimum ticks.
        assert_eq!(hw.seen(), &[10_000]);
        assert!(d.next_event_forced);
    }

    #[test]
    fn a_refused_minimum_delta_is_retried_then_grown() {
        let mut d = dev();
        let mut hw = Hw::new(3);
        assert_eq!(d.program_min_delta(&mut hw, 0), Ok(()));
        // Three refusals grow min_delta: 10_000 becomes 15_000.
        assert_eq!(d.min_delta_ns, 15_000);
        assert_eq!(hw.seen().len(), 4);
        assert!(d.retries >= 4);
    }

    #[test]
    fn minimum_delta_gives_up_at_the_limit() {
        let mut d = dev();
        d.min_delta_ns = MIN_DELTA_LIMIT;
        let mut hw = Hw::new(u32::MAX);
        assert_eq!(d.program_min_delta(&mut hw, 0), Err(Etime));
        assert_eq!(d.next_event, i64::MAX);
    }

    #[test]
    fn a_shut_down_device_is_not_programmed() {
        let mut d = ClockEventDevice {
            shutdown: true,
            ..dev()
        };
        let mut hw = Hw::new(0);
        assert_eq!(d.program_event(&mut hw, 50_000, 0, false), Ok(()));
        assert!(hw.seen().is_empty());
    }
}

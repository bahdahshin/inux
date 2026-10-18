// SPDX-License-Identifier: GPL-2.0+
//! A cycle counter turned into nanoseconds, with wrap handling.
//!
//! Ported from `kernel/time/timecounter.c` and the `cyclecounter_cyc2ns()`
//! helper in `include/linux/timecounter.h`. The C `read` callback receives
//! the counter; here it is a closure, since the callers keep the state in the
//! closure's captures. `timecounter_cyc2time()` is not ported: nothing in
//! this file uses it.

/// `struct cyclecounter`: a counter with its read function and its scale from
/// cycles to nanoseconds, `ns = cycles * mult >> shift`.
pub struct Cyclecounter<'a> {
    /// Returns the current cycle count.
    pub read: &'a dyn Fn() -> u64,
    /// Bits of the counter that are valid, so a wrap shows as a small delta.
    pub mask: u64,
    /// Scale numerator.
    pub mult: u32,
    /// Scale shift.
    pub shift: u32,
}

/// `struct timecounter`: the state that turns a wrapping counter into a
/// running nanosecond count.
pub struct Timecounter<'a> {
    cc: &'a Cyclecounter<'a>,
    cycle_last: u64,
    nsec: u64,
    mask: u64,
    frac: u64,
}

/// `cyclecounter_cyc2ns()`: convert `cycles` to nanoseconds, carrying the
/// bits below one nanosecond in `frac` between calls.
fn cyclecounter_cyc2ns(cc: &Cyclecounter<'_>, cycles: u64, mask: u64, frac: &mut u64) -> u64 {
    let ns = cycles.wrapping_mul(cc.mult as u64).wrapping_add(*frac);
    *frac = ns & mask;
    ns.checked_shr(cc.shift).unwrap_or(0)
}

impl<'a> Timecounter<'a> {
    /// `timecounter_init()`: start counting at `start_tstamp` nanoseconds.
    pub fn new(cc: &'a Cyclecounter<'a>, start_tstamp: u64) -> Self {
        let cycle_last = (cc.read)();
        let mask = 1u64.checked_shl(cc.shift).map_or(u64::MAX, |v| v - 1);
        Self {
            cc,
            cycle_last,
            nsec: start_tstamp,
            mask,
            frac: 0,
        }
    }

    /// `timecounter_read_delta()`: nanoseconds since the last read. A counter
    /// that wrapped once since then is handled by the masked difference.
    fn read_delta(&mut self) -> u64 {
        let cycle_now = (self.cc.read)();
        let cycle_delta = cycle_now.wrapping_sub(self.cycle_last) & self.cc.mask;
        let ns_offset = cyclecounter_cyc2ns(self.cc, cycle_delta, self.mask, &mut self.frac);
        self.cycle_last = cycle_now;
        ns_offset
    }

    /// `timecounter_read()`: the nanosecond count now.
    pub fn read(&mut self) -> u64 {
        let nsec = self.read_delta().wrapping_add(self.nsec);
        self.nsec = nsec;
        nsec
    }
}

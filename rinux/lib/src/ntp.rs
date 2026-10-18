// SPDX-License-Identifier: GPL-2.0
//! The NTP clock discipline: how a phase error turns into a frequency
//! correction, in PLL and FLL modes.
//!
//! Ported from `kernel/time/ntp.c`: `ntp_update_frequency()`,
//! `ntp_update_offset()` and `ntp_update_offset_fll()`, over the fields of
//! `struct ntp_data` that they use. The per-timekeeper array (`tkid`), the
//! locking, the leap-second and RTC-sync paths, the PPS discipline and
//! `adjtimex(2)` are not ported. The caller passes the seconds since the
//! last reference point, which C reads from `ktime_get_ntp_seconds()`.

/// `HZ`: the kernel tick rate rinux runs at.
pub const NTP_INTERVAL_FREQ: i64 = 1000;
/// `USER_HZ`: the tick rate user space sees.
pub const USER_HZ: u64 = 100;
/// `NTP_SCALE_SHIFT`: frequencies are scaled by 2^32.
pub const NTP_SCALE_SHIFT: u32 = 32;
/// `SHIFT_PLL`: the PLL frequency factor.
pub const SHIFT_PLL: u32 = 2;
/// `SHIFT_FLL`: the FLL frequency factor.
pub const SHIFT_FLL: u32 = 2;
/// `MAXPHASE`: the largest phase error, in nanoseconds.
pub const MAXPHASE: i64 = 500_000_000;
/// `MAXFREQ`: the largest frequency error, in ns per second.
pub const MAXFREQ: i64 = 500_000;
/// `MAXFREQ_SCALED`.
pub const MAXFREQ_SCALED: i64 = MAXFREQ << NTP_SCALE_SHIFT;
/// `MINSEC`: the shortest update interval the FLL uses.
pub const MINSEC: i64 = 256;
/// `MAXSEC`: the longest update interval the FLL uses.
pub const MAXSEC: i64 = 2048;

/// `STA_PLL`: enable PLL updates.
pub const STA_PLL: i32 = 0x0001;
/// `STA_FLL`: select the frequency-lock mode.
pub const STA_FLL: i32 = 0x0008;
/// `STA_FREQHOLD`: hold the frequency.
pub const STA_FREQHOLD: i32 = 0x0080;
/// `STA_NANO`: the phase is in nanoseconds, not microseconds.
pub const STA_NANO: i32 = 0x2000;
/// `STA_MODE`: set while in FLL mode.
pub const STA_MODE: i32 = 0x4000;

/// The NTP state this module reads and updates, one timekeeper's worth.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NtpData {
    /// `time_status`: the `STA_*` flags.
    pub time_status: i32,
    /// `time_reftime`: the seconds at the last offset update.
    pub time_reftime: i64,
    /// `time_freq`: the frequency correction, scaled by 2^32.
    pub time_freq: i64,
    /// `time_offset`: the phase correction per interval, scaled by 2^32.
    pub time_offset: i64,
    /// `time_constant`: the PLL time constant.
    pub time_constant: i32,
    /// `tick_usec`: the nominal tick length, in microseconds.
    pub tick_usec: i64,
    /// `tick_length`: the tick length, scaled by 2^32 nanoseconds.
    pub tick_length: u64,
    /// `ntp_tick_adj`: the adjtimex tick adjustment.
    pub ntp_tick_adj: i64,
    /// `cs_tick_adj`: the clocksource's skew adjustment.
    pub cs_tick_adj: i64,
}

/// `ntp_update_frequency()`: recompute the tick length from the frequency
/// and the adjustments, and apply it at once.
pub fn ntp_update_frequency(d: &mut NtpData) {
    let tick_usec = d.tick_usec as u64;
    let mut second_length = (tick_usec * 1000 * USER_HZ) << NTP_SCALE_SHIFT;
    second_length = second_length.wrapping_add(d.ntp_tick_adj as u64);
    second_length = second_length.wrapping_add(d.cs_tick_adj as u64);
    second_length = second_length.wrapping_add(d.time_freq as u64);
    d.tick_length = second_length / NTP_INTERVAL_FREQ as u64;
}

/// `ntp_update_offset_fll()`: the frequency correction in FLL mode, which is
/// zero unless the interval is long enough (or the loop is already in FLL).
fn ntp_update_offset_fll(d: &mut NtpData, offset64: i64, secs: i64) -> i64 {
    d.time_status &= !STA_MODE;

    if secs < MINSEC {
        return 0;
    }
    if d.time_status & STA_FLL == 0 && secs <= MAXSEC {
        return 0;
    }

    d.time_status |= STA_MODE;
    (offset64 << (NTP_SCALE_SHIFT - SHIFT_FLL)) / secs
}

/// `ntp_update_offset()`: fold a phase error into the frequency and the phase
/// correction. `offset` is in microseconds unless `STA_NANO` is set, and
/// `real_secs` is the current NTP seconds.
pub fn ntp_update_offset(d: &mut NtpData, offset: i64, real_secs: i64) {
    if d.time_status & STA_PLL == 0 {
        return;
    }

    let mut offset = offset;
    if d.time_status & STA_NANO == 0 {
        // Keep the multiplication below from overflowing.
        offset = offset.clamp(-1_000_000, 1_000_000);
        offset *= 1000;
    }

    // Scale the phase adjustment and clamp it to the operating range.
    offset = offset.clamp(-MAXPHASE, MAXPHASE);

    // Choose the PLL or FLL update from the time since the last reference.
    let mut secs = real_secs - d.time_reftime;
    if d.time_status & STA_FREQHOLD != 0 {
        secs = 0;
    }
    d.time_reftime = real_secs;

    let offset64 = offset;
    let mut freq_adj = ntp_update_offset_fll(d, offset64, secs);

    // Clamp the update interval so a sparse source does not make the loop
    // unstable.
    let max_secs = 1i64 << (SHIFT_PLL + 1 + d.time_constant as u32);
    if secs > max_secs {
        secs = max_secs;
    }

    let shift = NTP_SCALE_SHIFT as i64 - 2 * (SHIFT_PLL as i64 + 2 + d.time_constant as i64);
    freq_adj += (offset64 * secs) << shift;

    freq_adj = (freq_adj + d.time_freq).min(MAXFREQ_SCALED);
    d.time_freq = freq_adj.max(-MAXFREQ_SCALED);

    d.time_offset = (offset64 << NTP_SCALE_SHIFT) / NTP_INTERVAL_FREQ;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pll() -> NtpData {
        NtpData {
            time_status: STA_PLL | STA_NANO,
            time_constant: 2,
            tick_usec: 10_000,
            ..NtpData::default()
        }
    }

    #[test]
    fn nothing_changes_without_pll() {
        let mut d = NtpData {
            time_status: STA_NANO,
            ..pll()
        };
        let before = d;
        ntp_update_offset(&mut d, 1000, 10);
        assert_eq!(d, before);
    }

    #[test]
    fn a_positive_offset_raises_the_frequency() {
        let mut d = pll();
        ntp_update_offset(&mut d, 1_000, 1);
        assert!(d.time_freq > 0);
        assert_eq!(d.time_reftime, 1);
    }

    #[test]
    fn a_negative_offset_lowers_the_frequency() {
        let mut d = pll();
        ntp_update_offset(&mut d, -1_000, 1);
        assert!(d.time_freq < 0);
    }

    #[test]
    fn microsecond_offsets_are_scaled_to_nanoseconds() {
        let mut us = NtpData {
            time_status: STA_PLL,
            ..pll()
        };
        let mut ns = pll();
        ntp_update_offset(&mut us, 5, 1);
        ntp_update_offset(&mut ns, 5_000, 1);
        assert_eq!(us.time_freq, ns.time_freq);
        assert_eq!(us.time_offset, ns.time_offset);
    }

    #[test]
    fn the_phase_error_is_clamped() {
        let mut d = pll();
        ntp_update_offset(&mut d, 10 * MAXPHASE, 1);
        let mut limit = pll();
        ntp_update_offset(&mut limit, MAXPHASE, 1);
        assert_eq!(d.time_offset, limit.time_offset);
    }

    #[test]
    fn the_frequency_is_clamped_to_maxfreq() {
        let mut d = pll();
        for t in 1..200 {
            ntp_update_offset(&mut d, MAXPHASE, t * 1000);
        }
        assert!(d.time_freq <= MAXFREQ_SCALED);
        assert!(d.time_freq >= -MAXFREQ_SCALED);
    }

    #[test]
    fn freq_hold_keeps_the_interval_at_zero() {
        let mut held = NtpData {
            time_status: STA_PLL | STA_NANO | STA_FREQHOLD,
            ..pll()
        };
        let mut free = pll();
        ntp_update_offset(&mut held, 1_000, 1000);
        ntp_update_offset(&mut free, 1_000, 1000);
        assert!(held.time_freq.abs() < free.time_freq.abs());
    }

    #[test]
    fn fll_engages_for_a_long_interval() {
        let mut d = NtpData {
            time_status: STA_PLL | STA_NANO | STA_FLL,
            ..pll()
        };
        ntp_update_offset(&mut d, 1_000, 0);
        ntp_update_offset(&mut d, 1_000, 512);
        assert_ne!(d.time_status & STA_MODE, 0);
        // A short interval leaves FLL mode again.
        ntp_update_offset(&mut d, 1_000, 520);
        assert_eq!(d.time_status & STA_MODE, 0);
    }

    #[test]
    fn tick_length_follows_the_frequency() {
        let mut lo = pll();
        let mut hi = pll();
        lo.time_freq = 0;
        hi.time_freq = 1 << 20;
        ntp_update_frequency(&mut lo);
        ntp_update_frequency(&mut hi);
        assert!(hi.tick_length > lo.tick_length);
        let base = ((10_000u64 * 1000 * USER_HZ) << NTP_SCALE_SHIFT) / NTP_INTERVAL_FREQ as u64;
        assert_eq!(lo.tick_length, base);
    }
}

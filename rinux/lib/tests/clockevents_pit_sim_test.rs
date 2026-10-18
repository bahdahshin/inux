// SPDX-License-Identifier: GPL-2.0-or-later
//! Simulates the one-shot PIT tick: the clock event device is programmed by
//! the tick handler, which runs late by a varying amount. Checks that after
//! every handler run the device has an event pending, so the tick never
//! stops. The handler matches the kernel's `rearm_tick()`, without the clamp.

use rinux_lib::clockevents::{ClockEventDevice, NSEC_PER_SEC, SetNextEvent};

const RATE: u64 = 1_193_182;
const PERIOD_NS: i64 = 1_000_000;

/// The PIT's one-shot channel: a count that fires once at its terminal count.
struct Pit {
    /// The time the pending one-shot fires, in ns, if one is armed.
    pending: Option<i64>,
    /// The simulated now, in ns, at the time of the last write.
    now: i64,
}

impl SetNextEvent for Pit {
    fn set_next_event(&mut self, cycles: u64) -> i32 {
        if cycles == 0 || cycles > 0xffff {
            return -1;
        }
        self.pending = Some(self.now + (cycles * NSEC_PER_SEC / RATE) as i64);
        0
    }
}

#[test]
fn the_tick_always_has_an_event_pending() {
    let mult = (RATE << 32) / NSEC_PER_SEC;
    let mut dev = ClockEventDevice {
        mult,
        shift: 32,
        min_delta_ns: 10_000,
        max_delta_ns: 60_000_000,
        min_delta_ticks: 0,
        next_event: 0,
        next_event_forced: false,
        retries: 0,
        shutdown: false,
    };
    let mut pit = Pit {
        pending: None,
        now: 0,
    };
    dev.program_event(&mut pit, PERIOD_NS, 0, true).unwrap();

    // A small LCG gives a repeatable spread of handler latencies, up to
    // about 1.5 periods, which covers the late-tick case.
    let mut seed: u64 = 0x2545_f491_4f6c_dd1d;
    let mut expected_tick = PERIOD_NS;
    for step in 0..20_000 {
        let fire = pit.pending.take().unwrap_or_else(|| {
            panic!("step {step}: no event pending, the tick has stopped");
        });
        dev.event_fired();
        seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let latency = ((seed >> 33) % 1_500_000) as i64;

        // The handler runs `latency` ns after the fire.
        let since_fire_ns = latency;
        pit.now = fire + since_fire_ns;
        expected_tick += PERIOD_NS;
        let expires = PERIOD_NS - since_fire_ns;
        let result = dev.program_event(&mut pit, expires, 0, true);

        assert!(
            pit.pending.is_some(),
            "step {step}: latency {latency} ns left the device with no event (next_event_forced {}, result {:?}, expires {expires}, min {})",
            dev.next_event_forced,
            result,
            dev.min_delta_ns
        );
        let _ = expected_tick;
    }
}

#[test]
fn a_stale_forced_flag_leaves_the_device_idle() {
    // The root cause: a forced (minimum-delta) event fires, but the handler
    // does not clear the flag, so the next program returns early and arms
    // nothing. This is what stalled the tick in the QEMU boot.
    let mult = (RATE << 32) / NSEC_PER_SEC;
    let mut dev = ClockEventDevice {
        mult,
        shift: 32,
        min_delta_ns: 10_000,
        max_delta_ns: 60_000_000,
        min_delta_ticks: 0,
        next_event: 0,
        next_event_forced: false,
        retries: 0,
        shutdown: false,
    };
    let mut pit = Pit {
        pending: None,
        now: 0,
    };
    // A late handler: the expiry is already in the past, so the forced path
    // arms the minimum delta and sets the flag.
    dev.program_event(&mut pit, -500_000, 0, true).unwrap();
    assert!(dev.next_event_forced);
    let fire = pit.pending.take().unwrap();
    pit.now = fire;

    // Handler without event_fired(): a deadline within the minimum delta
    // takes the forced path, which the stale flag short-circuits.
    dev.program_event(&mut pit, 5_000, 0, true).unwrap();
    assert!(
        pit.pending.is_none(),
        "stale flag must leave the device idle"
    );

    // Handler with event_fired(): the same deadline arms the minimum delta.
    dev.event_fired();
    dev.program_event(&mut pit, 5_000, 0, true).unwrap();
    assert!(pit.pending.is_some());
}

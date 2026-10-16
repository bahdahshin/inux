// SPDX-License-Identifier: GPL-2.0 OR Linux-OpenIB
//! Dynamic Interrupt Moderation: the sample arithmetic and the tuning state
//! machine shared by the network and RDMA users.
//!
//! Ported from `lib/dim/dim.c`, `lib/dim/rdma_dim.c` and the inline and macro
//! helpers of `include/linux/dim.h`. The C algorithm runs against a live
//! `ktime_get()` clock and a `schedule_work()` queue; here the timestamps are
//! passed in as nanoseconds, and [`Dim::rdma_dim`] returns `true` where C
//! calls `schedule_work()`, so the caller queues the work itself.
//!
//! The net DIM profile tables (`lib/dim/net_dim.c`) are not ported here.

/// `DIM_NEVENTS`: events to collect before the algorithm decides.
pub const DIM_NEVENTS: u32 = 64;

/// `RDMA_DIM_PARAMS_NUM_PROFILES`: profiles in the RDMA moderation table.
pub const RDMA_DIM_PARAMS_NUM_PROFILES: u8 = 9;

/// `RDMA_DIM_START_PROFILE`: the profile a new instance starts on.
pub const RDMA_DIM_START_PROFILE: u8 = 0;

/// `IS_SIGNIFICANT_DIFF(val, ref)`: a change of more than 10% against a
/// non-zero reference. A zero reference is never significant.
pub const fn is_significant_diff(val: i64, reference: i64) -> bool {
    reference != 0 && (100 * (val - reference).unsigned_abs()) / (reference.unsigned_abs()) > 10
}

/// `BIT_GAP(bits, end, start)`: the distance between two counters that may
/// have wrapped at `bits` bits. For 32 bits this is `end - start` in `u32`.
pub const fn bit_gap32(end: u32, start: u32) -> u32 {
    end.wrapping_sub(start)
}

/// `DIV_ROUND_UP()` on unsigned operands, with C's wrapping sum.
const fn div_round_up(n: u32, d: u32) -> u32 {
    n.wrapping_add(d - 1) / d
}

/// `enum dim_state`: whether a measurement is running.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DimState {
    /// The first iteration, or the one after a new profile was applied.
    #[default]
    StartMeasure,
    /// A measurement is running; check it when enough events arrived.
    MeasureInProgress,
    /// The consumer is applying a profile; do not measure.
    ApplyNewProfile,
}

/// `enum dim_tune_state`: where the walk over profiles is.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DimTuneState {
    /// A local top was found; leave on a significant difference.
    #[default]
    ParkingOnTop,
    /// A deep top was found; leave only when tired.
    ParkingTired,
    /// Trying higher moderation levels.
    GoingRight,
    /// Trying lower moderation levels.
    GoingLeft,
}

/// `enum dim_stats_state`: the result of comparing two measurements.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DimStatsState {
    /// Worse than before.
    Worse,
    /// The same as before.
    Same,
    /// Better than before.
    Better,
}

/// `enum dim_step_result`: the result of moving one profile.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DimStepResult {
    /// A regular step was taken.
    Stepped,
    /// The walk reached the first or last profile.
    OnEdge,
}

/// `struct dim_sample`: a snapshot of the counters at one instant.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DimSample {
    /// Sample timestamp, in nanoseconds (`ktime_t`).
    pub time: u64,
    /// Packets seen so far.
    pub pkt_ctr: u32,
    /// Bytes seen so far.
    pub byte_ctr: u32,
    /// Events seen so far; wraps at 16 bits.
    pub event_ctr: u16,
    /// Completions seen so far.
    pub comp_ctr: u32,
}

/// `struct dim_stats`: the rates measured over one interval. The C fields
/// are `int`; they are kept as `i32` here.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DimStats {
    /// Packets per millisecond.
    pub ppms: i32,
    /// Bytes per millisecond.
    pub bpms: i32,
    /// Events per millisecond.
    pub epms: i32,
    /// Completions per millisecond.
    pub cpms: i32,
    /// Completions per event, times 100.
    pub cpe_ratio: i32,
}

/// `struct dim`: one moderation instance.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Dim {
    /// Whether a measurement is running.
    pub state: DimState,
    /// Rates from the previous iteration, for comparison.
    pub prev_stats: DimStats,
    /// Counters at the start of the current iteration.
    pub start_sample: DimSample,
    /// Counters updated by each event.
    pub measuring_sample: DimSample,
    /// Index of the current moderation profile.
    pub profile_ix: u8,
    /// Tuning state.
    pub tune_state: DimTuneState,
    /// Steps taken towards higher moderation since the last turn.
    pub steps_right: u8,
    /// Steps taken towards lower moderation since the last turn.
    pub steps_left: u8,
    /// Parking depth counter.
    pub tired: u8,
}

/// `dim_update_sample()`: set the timestamp and counters of a sample. The
/// completion counter is left as it was; see [`update_sample_with_comps`].
pub fn update_sample(time: u64, event_ctr: u16, packets: u64, bytes: u64, s: &mut DimSample) {
    s.time = time;
    s.pkt_ctr = packets as u32;
    s.byte_ctr = bytes as u32;
    s.event_ctr = event_ctr;
}

/// `dim_update_sample_with_comps()`: [`update_sample`] plus the completion
/// counter, truncated to 32 bits as the C field is.
pub fn update_sample_with_comps(
    time: u64,
    event_ctr: u16,
    packets: u64,
    bytes: u64,
    comps: u64,
    s: &mut DimSample,
) {
    update_sample(time, event_ctr, packets, bytes, s);
    s.comp_ctr = comps as u32;
}

/// `dim_on_top()`: whether the walk is at a local top, so the next step
/// should park rather than keep going.
pub fn dim_on_top(dim: &Dim) -> bool {
    match dim.tune_state {
        DimTuneState::ParkingOnTop | DimTuneState::ParkingTired => true,
        DimTuneState::GoingRight => dim.steps_left > 1 && dim.steps_right == 1,
        DimTuneState::GoingLeft => dim.steps_right > 1 && dim.steps_left == 1,
    }
}

/// `dim_turn()`: reverse direction. A parked instance stays parked.
pub fn dim_turn(dim: &mut Dim) {
    match dim.tune_state {
        DimTuneState::ParkingOnTop | DimTuneState::ParkingTired => {}
        DimTuneState::GoingRight => {
            dim.tune_state = DimTuneState::GoingLeft;
            dim.steps_left = 0;
        }
        DimTuneState::GoingLeft => {
            dim.tune_state = DimTuneState::GoingRight;
            dim.steps_right = 0;
        }
    }
}

/// `dim_park_on_top()`: stop at a local top and forget the walk.
pub fn dim_park_on_top(dim: &mut Dim) {
    dim.steps_right = 0;
    dim.steps_left = 0;
    dim.tired = 0;
    dim.tune_state = DimTuneState::ParkingOnTop;
}

/// `dim_park_tired()`: park at a deep top; only tiredness brings it back.
pub fn dim_park_tired(dim: &mut Dim) {
    dim.steps_right = 0;
    dim.steps_left = 0;
    dim.tune_state = DimTuneState::ParkingTired;
}

/// `dim_calc_stats()`: the rates between two samples. `None` when no time
/// passed, where C returns false. The time difference is taken in whole
/// microseconds and truncated to `u32`, as the C code does.
pub fn dim_calc_stats(start: &DimSample, end: &DimSample) -> Option<DimStats> {
    let delta_us = ((end.time as i64 - start.time as i64) / 1000) as u32;
    let npkts = bit_gap32(end.pkt_ctr, start.pkt_ctr);
    let nbytes = bit_gap32(end.byte_ctr, start.byte_ctr);
    let ncomps = bit_gap32(end.comp_ctr, start.comp_ctr);

    if delta_us == 0 {
        return None;
    }

    let ppms = div_round_up(npkts.wrapping_mul(1000), delta_us) as i32;
    let bpms = div_round_up(nbytes.wrapping_mul(1000), delta_us) as i32;
    let epms = div_round_up(DIM_NEVENTS * 1000, delta_us) as i32;
    let cpms = div_round_up(ncomps.wrapping_mul(1000), delta_us) as i32;
    let cpe_ratio = if epms != 0 {
        ((cpms as u64 * 100) / epms as u64) as i32
    } else {
        0
    };

    Some(DimStats {
        ppms,
        bpms,
        epms,
        cpms,
        cpe_ratio,
    })
}

/// `rdma_dim_step()`: move one profile in the current direction.
fn rdma_dim_step(dim: &mut Dim) -> DimStepResult {
    if dim.tune_state == DimTuneState::GoingRight {
        if dim.profile_ix == RDMA_DIM_PARAMS_NUM_PROFILES - 1 {
            return DimStepResult::OnEdge;
        }
        dim.profile_ix += 1;
        dim.steps_right += 1;
    }
    if dim.tune_state == DimTuneState::GoingLeft {
        if dim.profile_ix == 0 {
            return DimStepResult::OnEdge;
        }
        dim.profile_ix -= 1;
        dim.steps_left += 1;
    }
    DimStepResult::Stepped
}

/// `rdma_dim_stats_compare()`: compare the completion rate first, then the
/// completions per event. The first measurement (no previous completions
/// rate) counts as the same.
fn rdma_dim_stats_compare(curr: &DimStats, prev: &DimStats) -> DimStatsState {
    if prev.cpms == 0 {
        return DimStatsState::Same;
    }
    if is_significant_diff(curr.cpms as i64, prev.cpms as i64) {
        return if curr.cpms > prev.cpms {
            DimStatsState::Better
        } else {
            DimStatsState::Worse
        };
    }
    if is_significant_diff(curr.cpe_ratio as i64, prev.cpe_ratio as i64) {
        return if curr.cpe_ratio > prev.cpe_ratio {
            DimStatsState::Better
        } else {
            DimStatsState::Worse
        };
    }
    DimStatsState::Same
}

/// `rdma_dim_decision()`: pick the next profile from the latest rates.
/// Returns whether the profile index changed.
fn rdma_dim_decision(curr_stats: &DimStats, dim: &mut Dim) -> bool {
    let prev_ix = dim.profile_ix;
    let state = dim.tune_state;

    if state != DimTuneState::ParkingOnTop && state != DimTuneState::ParkingTired {
        match rdma_dim_stats_compare(curr_stats, &dim.prev_stats) {
            DimStatsState::Same => {
                if curr_stats.cpe_ratio <= 50 * prev_ix as i32 {
                    dim.profile_ix = 0;
                }
            }
            DimStatsState::Worse => {
                dim_turn(dim);
                if rdma_dim_step(dim) == DimStepResult::OnEdge {
                    dim_turn(dim);
                }
            }
            DimStatsState::Better => {
                if rdma_dim_step(dim) == DimStepResult::OnEdge {
                    dim_turn(dim);
                }
            }
        }
    }

    dim.prev_stats = *curr_stats;
    dim.profile_ix != prev_ix
}

impl Dim {
    /// A new instance as `drivers/infiniband/core/cq.c` sets one up: start a
    /// measurement on the start profile, and walk towards higher moderation.
    pub fn new() -> Self {
        Dim {
            state: DimState::StartMeasure,
            tune_state: DimTuneState::GoingRight,
            profile_ix: RDMA_DIM_START_PROFILE,
            ..Dim::default()
        }
    }

    /// `rdma_dim()`: count one event with `completions` completions and, once
    /// enough events were seen, decide on a new profile. Returns `true` when
    /// a new profile was chosen; C then calls `schedule_work()`, and the
    /// caller queues the work.
    pub fn rdma_dim(&mut self, time: u64, completions: u64) -> bool {
        let curr = self.measuring_sample;
        update_sample_with_comps(
            time,
            curr.event_ctr.wrapping_add(1),
            0,
            0,
            curr.comp_ctr as u64 + completions,
            &mut self.measuring_sample,
        );
        let curr_sample = self.measuring_sample;

        match self.state {
            DimState::MeasureInProgress => {
                // C `break`s out of the switch on each of these early exits,
                // so the start sample is kept until the measurement is done.
                let nevents =
                    (curr_sample.event_ctr as i32 - self.start_sample.event_ctr as i32) as u32;
                if nevents < DIM_NEVENTS {
                    return false;
                }
                let Some(curr_stats) = dim_calc_stats(&self.start_sample, &curr_sample) else {
                    return false;
                };
                if rdma_dim_decision(&curr_stats, self) {
                    self.state = DimState::ApplyNewProfile;
                    return true;
                }
                // No new profile: fall through to the start of the next measurement.
                self.start_measure(time, curr_sample);
                false
            }
            DimState::StartMeasure => {
                self.start_measure(time, curr_sample);
                false
            }
            DimState::ApplyNewProfile => false,
        }
    }

    /// The `DIM_START_MEASURE` arm: `dim_update_sample_with_comps()` stamps
    /// the start sample with the current time and the counters of `curr`.
    fn start_measure(&mut self, time: u64, curr: DimSample) {
        self.state = DimState::MeasureInProgress;
        update_sample_with_comps(
            time,
            curr.event_ctr,
            0,
            0,
            curr.comp_ctr as u64,
            &mut self.start_sample,
        );
    }
}

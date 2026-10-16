// SPDX-License-Identifier: GPL-2.0-or-later
//! Host tests for the EEVDF accumulators, checked against exact rational
//! arithmetic over random entities: eligibility is `v <= weighted average`,
//! and the deadline moves by one request. Linux has no KUnit file for these.

use rinux_lib::random32::RndState;
use rinux_lib::sched_eevdf::{EevdfQueue, update_deadline};
use rinux_lib::sched_fair::{NICE_0_LOAD, scale_load};

#[test]
fn eligibility_matches_the_exact_weighted_average() {
    let mut rnd = RndState::new(3);
    for _ in 0..200 {
        let zero = u64::from(rnd.next_u32()) << 8;
        let mut q = EevdfQueue::new(zero);
        let mut ents = Vec::new();
        for _ in 0..6 {
            let v = zero.wrapping_add(u64::from(rnd.next_u32()) % 4_000_000);
            let w = scale_load(1 + u64::from(rnd.next_u32()) % 90);
            q.enqueue(v, w);
            ents.push((v, w));
        }
        for &(probe, _) in &ents {
            // Exact: sum w*(v - zero) >= (probe - zero) * sum w.
            let mut lhs: i128 = 0;
            let mut w_sum: i128 = 0;
            for &(v, w) in &ents {
                lhs += i128::from(v.wrapping_sub(zero) as i64) * i128::from(w);
                w_sum += i128::from(w);
            }
            let rhs = i128::from(probe.wrapping_sub(zero) as i64) * w_sum;
            assert_eq!(q.vruntime_eligible(probe), lhs >= rhs);
        }
    }
}

#[test]
fn average_is_the_weighted_mean_of_the_entities() {
    let mut q = EevdfQueue::new(1000);
    q.enqueue(1100, 1024);
    q.enqueue(1300, 1024);
    assert_eq!(q.avg_vruntime(), 1200);
    // A heavier entity pulls the average towards its own vruntime.
    q.enqueue(1400, 3 * 1024);
    let avg = q.avg_vruntime();
    assert!((1300..=1400).contains(&avg), "avg {avg}");
}

#[test]
fn an_empty_queue_averages_to_its_zero_point() {
    assert_eq!(EevdfQueue::new(77).avg_vruntime(), 77);
}

#[test]
fn deadline_advances_by_one_request_once_used_up() {
    let slice = 700_000;
    // Not yet used up: unchanged.
    assert_eq!(update_deadline(100, 200, slice, NICE_0_LOAD), (false, 200));
    // Used up at nice 0: the next deadline is vruntime + slice.
    assert_eq!(
        update_deadline(300, 200, slice, NICE_0_LOAD),
        (true, 300 + slice)
    );
}

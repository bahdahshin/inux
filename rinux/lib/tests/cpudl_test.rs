// SPDX-License-Identifier: GPL-2.0-or-later
//! Host tests for the per-CPU deadline heap: the root is always the latest
//! deadline, set and clear keep it so, and find() answers against it. Linux has
//! no KUnit file for cpudeadline.c.

use rinux_lib::cpudl::{CpuDl, dl_time_before};
use rinux_lib::random32::RndState;

#[test]
fn the_root_is_the_latest_deadline() {
    let mut d: CpuDl<8> = CpuDl::new();
    d.set(0, 100);
    d.set(1, 300);
    d.set(2, 200);
    assert_eq!(d.maximum(), Some(1));
    d.clear(1, true);
    assert_eq!(d.maximum(), Some(2));
    assert!(d.is_free(1));
}

#[test]
fn find_says_whether_a_task_would_displace_the_root() {
    let mut d: CpuDl<4> = CpuDl::new();
    assert_eq!(d.find(5), None);
    d.set(0, 1000);
    assert_eq!(d.find(500), Some(0)); // earlier than 1000: displaces it
    assert_eq!(d.find(2000), None);
}

#[test]
fn random_updates_keep_the_root_at_the_brute_force_maximum() {
    let mut rnd = RndState::new(11);
    let mut d: CpuDl<16> = CpuDl::new();
    let mut live: [Option<u64>; 16] = [None; 16];
    for _ in 0..2000 {
        let cpu = (rnd.next_u32() % 16) as usize;
        if rnd.next_u32() % 3 == 0 {
            d.clear(cpu, true);
            live[cpu] = None;
        } else {
            let dl = u64::from(rnd.next_u32()) << 4;
            d.set(cpu, dl);
            live[cpu] = Some(dl);
        }
        let best = live
            .iter()
            .enumerate()
            .filter_map(|(c, v)| v.map(|dl| (c, dl)))
            .fold(None, |acc: Option<(usize, u64)>, (c, dl)| match acc {
                Some((_, adl)) if !dl_time_before(adl, dl) => acc,
                _ => Some((c, dl)),
            });
        match (d.maximum(), best) {
            (None, None) => {}
            (Some(root), Some((_, bdl))) => {
                let rdl = live[root].expect("root must be live");
                assert_eq!(rdl, bdl);
            }
            other => panic!("mismatch: {other:?}"),
        }
    }
}

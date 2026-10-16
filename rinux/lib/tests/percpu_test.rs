// SPDX-License-Identifier: GPL-2.0-or-later
//! Host tests for the per-CPU slot array. Linux has no KUnit file for the
//! data layout; these check that each CPU's slot is independent.

use rinux_lib::percpu::PerCpu;

#[test]
fn each_cpu_starts_with_its_own_value() {
    let p: PerCpu<u32, 4> = PerCpu::new(|cpu| cpu as u32 * 10);
    assert_eq!(p.cpus(), 4);
    for cpu in 0..4 {
        // SAFETY: the test is single-threaded and touches one slot at a time.
        let v = unsafe { p.with_cpu(cpu, |v| *v) };
        assert_eq!(v, cpu as u32 * 10);
    }
}

#[test]
fn get_mut_changes_only_its_own_slot() {
    let mut p: PerCpu<u64, 3> = PerCpu::new(|_| 0);
    *p.get_mut(1) += 5;
    assert_eq!(*p.get_mut(0), 0);
    assert_eq!(*p.get_mut(1), 5);
    assert_eq!(*p.get_mut(2), 0);
}

#[test]
fn with_cpu_updates_the_slot_it_names() {
    let p: PerCpu<i32, 2> = PerCpu::new(|_| 1);
    // SAFETY: single-threaded test; each call names one slot at a time.
    unsafe {
        p.with_cpu(0, |v| *v += 2);
        p.with_cpu(1, |v| *v *= 7);
    }
    let (a, b) = unsafe { (p.with_cpu(0, |v| *v), p.with_cpu(1, |v| *v)) };
    assert_eq!((a, b), (3, 7));
}

#[test]
#[should_panic]
fn a_cpu_past_the_end_panics() {
    let mut p: PerCpu<u8, 2> = PerCpu::new(|_| 0);
    let _ = p.get_mut(2);
}

static STATIC_SLOTS: PerCpu<u32, 2> = PerCpu::from_array([7, 9]);

#[test]
fn from_array_works_in_a_static() {
    // SAFETY: single-threaded test; each call names one slot at a time.
    let (a, b) = unsafe {
        (
            STATIC_SLOTS.with_cpu(0, |v| *v),
            STATIC_SLOTS.with_cpu(1, |v| *v),
        )
    };
    assert_eq!((a, b), (7, 9));
}

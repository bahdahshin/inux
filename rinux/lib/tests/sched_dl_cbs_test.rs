// SPDX-License-Identifier: GPL-2.0-or-later
//! Host tests for the deadline server rules: replenishment by whole periods,
//! the new-period reset, and the overflow test against exact bandwidth.
//! Linux has no KUnit file for these.

use rinux_lib::sched_dl_cbs::*;

const P: DlParams = DlParams {
    dl_deadline: 100_000,
    dl_runtime: 20_000,
    dl_period: 100_000,
};

#[test]
fn a_fresh_entity_starts_a_new_period() {
    let mut se = DlEntity {
        deadline: 0,
        runtime: 0,
    };
    replenish_dl_entity(&mut se, &P, 1_000, false);
    assert_eq!(
        se,
        DlEntity {
            deadline: 101_000,
            runtime: 20_000
        }
    );
}

#[test]
fn an_overrun_is_paid_back_in_whole_periods() {
    // Overrun by 5000: one refill of 20000 makes the budget positive.
    let mut one = DlEntity {
        deadline: 200_000,
        runtime: -5_000,
    };
    replenish_dl_entity(&mut one, &P, 150_000, false);
    assert_eq!(
        one,
        DlEntity {
            deadline: 300_000,
            runtime: 15_000
        }
    );

    // Overrun by 25000: two refills, moving the deadline two periods.
    let mut two = DlEntity {
        deadline: 200_000,
        runtime: -25_000,
    };
    replenish_dl_entity(&mut two, &P, 150_000, false);
    assert_eq!(
        two,
        DlEntity {
            deadline: 400_000,
            runtime: 15_000
        }
    );
}

#[test]
fn a_lagging_deadline_restarts_the_period() {
    let mut se = DlEntity {
        deadline: 10_000,
        runtime: 1,
    };
    replenish_dl_entity(&mut se, &P, 500_000, false);
    assert_eq!(
        se,
        DlEntity {
            deadline: 600_000,
            runtime: 20_000
        }
    );
}

#[test]
fn a_task_within_its_bandwidth_does_not_overflow() {
    // 20% bandwidth: 10_000 left over 50_000 of time is 20%.
    let se = DlEntity {
        deadline: 150_000,
        runtime: 10_000,
    };
    assert!(!dl_entity_overflow(&se, &P, 100_000));
}

#[test]
fn a_task_over_its_bandwidth_overflows() {
    // 15_000 left over 50_000 is 30%, above the declared 20%.
    let se = DlEntity {
        deadline: 150_000,
        runtime: 15_000,
    };
    assert!(dl_entity_overflow(&se, &P, 100_000));
}

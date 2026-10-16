// SPDX-License-Identifier: GPL-2.0-or-later
//! Host tests for `current_is_single_threaded()`.

use rinux_lib::is_single_threaded::{Task, current_is_single_threaded};

fn leader(mm: Option<usize>, group: usize) -> Task {
    Task {
        mm,
        kthread: false,
        group_leader: true,
        group,
    }
}

fn thread(mm: Option<usize>, group: usize) -> Task {
    Task {
        mm,
        kthread: false,
        group_leader: false,
        group,
    }
}

#[test]
fn group_with_other_live_threads_is_not_single() {
    let tasks = [leader(Some(1), 0), thread(Some(1), 0)];
    assert!(!current_is_single_threaded(&tasks, 0, 2, 2));
}

#[test]
fn sole_user_of_mm_is_single() {
    let tasks = [leader(Some(1), 0)];
    assert!(current_is_single_threaded(&tasks, 0, 1, 1));
}

#[test]
fn other_process_sharing_mm_is_not_single() {
    let tasks = [leader(Some(1), 0), leader(Some(1), 1)];
    assert!(!current_is_single_threaded(&tasks, 0, 1, 2));
}

#[test]
fn other_process_with_different_mm_is_single() {
    let tasks = [leader(Some(1), 0), leader(Some(2), 1)];
    assert!(current_is_single_threaded(&tasks, 0, 1, 2));
}

#[test]
fn kernel_thread_sharing_mm_is_ignored() {
    let mut kt = leader(Some(1), 1);
    kt.kthread = true;
    let tasks = [leader(Some(1), 0), kt];
    assert!(current_is_single_threaded(&tasks, 0, 1, 2));
}

#[test]
fn kernel_task_without_mm_is_single() {
    let tasks = [leader(None, 0)];
    assert!(current_is_single_threaded(&tasks, 0, 1, 0));
}

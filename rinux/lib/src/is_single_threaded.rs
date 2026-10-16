// SPDX-License-Identifier: GPL-2.0-or-later
//! Whether a task is the only user of its address space.
//!
//! A port of `lib/is_single_threaded.c`, `current_is_single_threaded()`.
//! The C walks the live task list under RCU. This port takes the same walk
//! over a slice of [`Task`] records in list order, one per task, so the
//! caller supplies the process table and the thread group counts.
//!
//! The caller passes the current task's index, its thread group's live count
//! and its address space's user count, which the C reads from `task->signal`
//! and `mm->mm_users`.

/// The fields of `task_struct` this check reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Task {
    /// `task->mm`: the address-space id, or `None` for a kernel thread.
    pub mm: Option<usize>,
    /// `PF_KTHREAD`.
    pub kthread: bool,
    /// Whether this task is the leader of its thread group.
    pub group_leader: bool,
    /// The thread group this task belongs to.
    pub group: usize,
}

/// `current_is_single_threaded()`.
///
/// `current` is the index of the calling task in `tasks`; `live` is the
/// thread group's live count and `mm_users` the address space's user count.
pub fn current_is_single_threaded(
    tasks: &[Task],
    current: usize,
    live: u32,
    mm_users: u32,
) -> bool {
    let task = tasks[current];
    if live != 1 {
        return false;
    }
    let Some(mm) = task.mm else {
        // A task without an address space has no mm to share.
        return true;
    };
    if mm_users == 1 {
        return true;
    }
    // The inner `goto found` fires for a thread that shares the mm; `break`
    // ends the thread walk at the first thread that has any mm.
    for (p_idx, p) in tasks.iter().enumerate() {
        if p.kthread || !p.group_leader || p_idx == current_group_leader(tasks, current) {
            continue;
        }
        for t in tasks.iter().filter(|t| t.group == p.group) {
            if t.mm == Some(mm) {
                return false;
            }
            if t.mm.is_some() {
                break;
            }
        }
    }
    true
}

/// Index of the calling task's thread group leader.
fn current_group_leader(tasks: &[Task], current: usize) -> usize {
    let group = tasks[current].group;
    tasks
        .iter()
        .position(|t| t.group == group && t.group_leader)
        .unwrap_or(current)
}

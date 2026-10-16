// SPDX-License-Identifier: GPL-2.0-only
//! Map a task priority onto the CPU-priority scale that cpupri tracks.
//!
//! Ported from `kernel/sched/cpupri.c`, `convert_prio()`, and the constants of
//! `kernel/sched/cpupri.h`. The scale runs from INVALID (-1, not routable)
//! through NORMAL (0) and the real-time levels 1 to 99, to HIGHER (100). A
//! task's kernel priority (0 is the highest real-time) is turned around, so a
//! higher number here means a more urgent CPU state. The two-dimensional bitmap
//! that cpupri keeps over these levels, and the search over it, are not ported
//! here.

/// `CPUPRI_NR_PRIORITIES`: the number of levels.
pub const CPUPRI_NR_PRIORITIES: i32 = 101;
/// `CPUPRI_INVALID`: the CPU is not eligible for routing.
pub const CPUPRI_INVALID: i32 = -1;
/// `CPUPRI_NORMAL`: a CPU running only ordinary tasks.
pub const CPUPRI_NORMAL: i32 = 0;
/// `CPUPRI_HIGHER`: a CPU running a task more urgent than real-time.
pub const CPUPRI_HIGHER: i32 = 100;
/// `MAX_RT_PRIO`.
const MAX_RT_PRIO: i32 = 100;

/// `convert_prio()`: the cpupri level for a task priority, or `None` for a
/// priority C's switch does not cover (below -1 or above 100).
pub fn convert_prio(prio: i32) -> Option<i32> {
    match prio {
        CPUPRI_INVALID => Some(CPUPRI_INVALID),
        0..=98 => Some(MAX_RT_PRIO - 1 - prio),
        x if x == MAX_RT_PRIO - 1 => Some(CPUPRI_NORMAL),
        x if x == MAX_RT_PRIO => Some(CPUPRI_HIGHER),
        _ => None,
    }
}

/// `struct cpupri` over `N` CPUs: which CPUs sit at each level, and the level
/// each CPU is at.
#[derive(Debug)]
pub struct CpuPri<const N: usize> {
    /// `cpu_to_pri`: each CPU's current level.
    cpu_to_pri: [i32; N],
    /// `pri_to_cpu[level].mask` and `.count`: membership, per level.
    members: [[bool; N]; CPUPRI_NR_PRIORITIES as usize],
    counts: [usize; CPUPRI_NR_PRIORITIES as usize],
}

impl<const N: usize> Default for CpuPri<N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize> CpuPri<N> {
    /// `cpupri_init()`: every CPU starts INVALID, so none is routable.
    pub fn new() -> Self {
        Self {
            cpu_to_pri: [CPUPRI_INVALID; N],
            members: [[false; N]; CPUPRI_NR_PRIORITIES as usize],
            counts: [0; CPUPRI_NR_PRIORITIES as usize],
        }
    }

    /// `cpupri_set()`: move `cpu` to the level of task priority `newpri`.
    /// Panics if `newpri` is outside the scale, as C's BUG_ON does.
    pub fn set(&mut self, cpu: usize, newpri: i32) {
        let newpri = convert_prio(newpri).expect("task priority outside the cpupri scale");
        let oldpri = self.cpu_to_pri[cpu];
        if newpri == oldpri {
            return;
        }
        if newpri != CPUPRI_INVALID {
            let l = newpri as usize;
            self.members[l][cpu] = true;
            self.counts[l] += 1;
        }
        if oldpri != CPUPRI_INVALID {
            let l = oldpri as usize;
            self.members[l][cpu] = false;
            self.counts[l] -= 1;
        }
        self.cpu_to_pri[cpu] = newpri;
    }

    /// The level `cpu` is at.
    pub fn level(&self, cpu: usize) -> i32 {
        self.cpu_to_pri[cpu]
    }

    /// `cpupri_find()` without a fitness function: the first CPU, in order,
    /// that is at a level below `task_prio`'s and allowed by `allowed`. The
    /// levels are searched from the lowest up, as C does, so the least urgent
    /// CPU that can take the task is chosen.
    pub fn find(&self, task_prio: i32, allowed: &[bool; N]) -> Option<usize> {
        let task_level = convert_prio(task_prio)?;
        for idx in 0..task_level.max(0) as usize {
            if self.counts[idx] == 0 {
                continue;
            }
            if let Some(cpu) = (0..N).find(|&c| self.members[idx][c] && allowed[c]) {
                return Some(cpu);
            }
        }
        None
    }
}

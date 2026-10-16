// SPDX-License-Identifier: GPL-2.0-only
//! The per-CPU deadline heap: which CPU is running the latest-deadline
//! SCHED_DEADLINE task, so a new deadline task can find a CPU to push to.
//!
//! Ported from `kernel/sched/cpudeadline.c`: `cpudl_set()`, `cpudl_clear()`,
//! `cpudl_maximum()` and the heapify functions, with the deadline comparison
//! `dl_time_before()` as `(a - b) < 0` on the wrapping difference. The heap is
//! a max-heap by "later deadline", so the root is the CPU whose current task
//! has the most slack. The free-CPU mask is a bool per CPU, and the lock is the
//! caller's. Heap slots are indices into fixed arrays, so nothing is allocated.

const IDX_INVALID: usize = usize::MAX;

/// `dl_time_before()`: whether deadline `a` is earlier than `b`.
pub fn dl_time_before(a: u64, b: u64) -> bool {
    (a.wrapping_sub(b) as i64) < 0
}

/// `struct cpudl` over `N` CPUs.
#[derive(Debug)]
pub struct CpuDl<const N: usize> {
    size: usize,
    /// Heap slots: the CPU and its deadline, as `cpudl_item`.
    cpu: [usize; N],
    dl: [u64; N],
    /// Each CPU's slot in the heap, or `IDX_INVALID`.
    idx: [usize; N],
    /// `free_cpus`: CPUs with no deadline task.
    free: [bool; N],
}

impl<const N: usize> Default for CpuDl<N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize> CpuDl<N> {
    /// `cpudl_init()`: no CPU has a deadline task.
    pub fn new() -> Self {
        Self {
            size: 0,
            cpu: [0; N],
            dl: [0; N],
            idx: [IDX_INVALID; N],
            free: [false; N],
        }
    }

    /// Mark a CPU online (`free = true`) or offline, as `cpudl_clear()` does
    /// when the CPU has no deadline task.
    pub fn set_free(&mut self, cpu: usize, free: bool) {
        self.free[cpu] = free;
    }

    /// Whether a CPU is free of deadline tasks.
    pub fn is_free(&self, cpu: usize) -> bool {
        self.free[cpu]
    }

    /// The CPU at the root: the latest deadline in the system, if any.
    pub fn maximum(&self) -> Option<usize> {
        (self.size > 0).then(|| self.cpu[0])
    }

    /// `cpudl_find()` without a mask: the root CPU when a task with deadline
    /// `dl` is earlier than the root's deadline, so the task would displace it.
    pub fn find(&self, dl: u64) -> Option<usize> {
        let top = self.maximum()?;
        dl_time_before(dl, self.dl[0]).then_some(top)
    }

    fn put(&mut self, slot: usize, cpu: usize, dl: u64) {
        self.cpu[slot] = cpu;
        self.dl[slot] = dl;
        self.idx[cpu] = slot;
    }

    fn heapify_up(&mut self, mut slot: usize) {
        if slot == 0 {
            return;
        }
        let (orig_cpu, orig_dl) = (self.cpu[slot], self.dl[slot]);
        loop {
            let p = (slot - 1) >> 1;
            if dl_time_before(orig_dl, self.dl[p]) {
                break;
            }
            let (pc, pd) = (self.cpu[p], self.dl[p]);
            self.put(slot, pc, pd);
            slot = p;
            if slot == 0 {
                break;
            }
        }
        self.put(slot, orig_cpu, orig_dl);
    }

    fn heapify_down(&mut self, mut slot: usize) {
        if 2 * slot + 1 >= self.size {
            return;
        }
        let (orig_cpu, orig_dl) = (self.cpu[slot], self.dl[slot]);
        loop {
            let l = 2 * slot + 1;
            let r = 2 * slot + 2;
            let mut largest = slot;
            let mut largest_dl = orig_dl;
            if l < self.size && dl_time_before(orig_dl, self.dl[l]) {
                largest = l;
                largest_dl = self.dl[l];
            }
            if r < self.size && dl_time_before(largest_dl, self.dl[r]) {
                largest = r;
            }
            if largest == slot {
                break;
            }
            let (lc, ld) = (self.cpu[largest], self.dl[largest]);
            self.put(slot, lc, ld);
            slot = largest;
        }
        self.put(slot, orig_cpu, orig_dl);
    }

    fn heapify(&mut self, slot: usize) {
        if slot > 0 && dl_time_before(self.dl[(slot - 1) >> 1], self.dl[slot]) {
            self.heapify_up(slot);
        } else {
            self.heapify_down(slot);
        }
    }

    /// `cpudl_set()`: record that `cpu` now runs a task with deadline `dl`.
    pub fn set(&mut self, cpu: usize, dl: u64) {
        let old = self.idx[cpu];
        if old == IDX_INVALID {
            let slot = self.size;
            self.size += 1;
            self.put(slot, cpu, dl);
            self.heapify_up(slot);
            self.free[cpu] = false;
        } else {
            self.dl[old] = dl;
            self.heapify(old);
        }
    }

    /// `cpudl_clear()`: `cpu` has no deadline task. Removes it from the heap and
    /// marks it free if `online`.
    pub fn clear(&mut self, cpu: usize, online: bool) {
        let old = self.idx[cpu];
        if old != IDX_INVALID {
            let last = self.size - 1;
            let (lc, ld) = (self.cpu[last], self.dl[last]);
            self.put(old, lc, ld);
            self.size -= 1;
            self.idx[cpu] = IDX_INVALID;
            if old < self.size {
                self.heapify(old);
            }
        }
        self.free[cpu] = online;
    }
}

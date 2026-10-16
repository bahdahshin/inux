// SPDX-License-Identifier: GPL-2.0-only
//! The scheduler core: runqueue, context switch, `schedule()` and the tick.
//!
//! A partial port of `kernel/sched/core.c`. rinux schedules kernel threads
//! round-robin: each runnable task gets a time slice of `RR_TIMESLICE`
//! ticks, and when it runs out the task goes to the tail of the runqueue
//! and another is picked, as the real-time round-robin class does. The
//! idle task (`swapper/0`) runs only when nothing else is runnable.
//!
//! Simplified from Linux's: one CPU, one runqueue, no CFS/EEVDF, no
//! priorities or nice levels, no load balancing, no sleeping/wait queues
//! yet (tasks are either runnable or dead), and `mm` switching is skipped
//! because every task shares the kernel page tables.
//!
//! Copyright (C) 1991-2002  Linus Torvalds

use core::sync::atomic::{AtomicBool, AtomicPtr, AtomicU32, AtomicU64, Ordering};

use rinux_lib::list::{ListHead, iter, list_add_tail, list_del_init, list_entry};

use crate::arch::x86::include::irqflags::{local_irq_restore, local_irq_save};
use crate::arch::x86::kernel::process::{copy_thread, free_thread_stack, switch_to_asm};
use crate::include::linux::sched::*;
use crate::kernel::locking::{RacyCell, SpinLock};
use crate::kernel::softirq::in_interrupt;
use crate::kernel::time::jiffies::HZ;
use crate::mm::__GFP_ZERO;

/// `RR_TIMESLICE`: 100 ms worth of ticks.
pub const RR_TIMESLICE: u32 = (100 * HZ / 1000) as u32;

/// The current round-robin slice in ticks (`sched_rr_timeslice`).
static RR_SLICE: AtomicU32 = AtomicU32::new(RR_TIMESLICE);

/// Sets the round-robin slice (`sched_rr_timeslice`). Used by tests.
pub fn set_rr_timeslice(ticks: u32) {
    RR_SLICE.store(ticks.max(1), Ordering::Relaxed);
}

fn rr_timeslice() -> u32 {
    RR_SLICE.load(Ordering::Relaxed)
}

const RUN_LIST_OFFSET: usize = core::mem::offset_of!(TaskStruct, run_list);

/// `struct rq`: the per-CPU runqueue. Holds the runnable tasks (not the
/// idle task) in round-robin order.
struct Rq {
    queue: ListHead,
    nr_running: u64,
}

// SAFETY: the runqueue's task pointers are only followed under RQ's lock.
unsafe impl Send for Rq {}

static RQ: SpinLock<Rq> = SpinLock::new(Rq {
    queue: ListHead::new(),
    nr_running: 0,
});

/// `rq->idle`: the idle task.
static IDLE_TASK: AtomicPtr<TaskStruct> = AtomicPtr::new(core::ptr::null_mut());

/// `tif_need_resched()`: a reschedule is pending on this CPU.
static NEED_RESCHED: AtomicBool = AtomicBool::new(false);

/// Previous task awaiting finish after the last switch (`rq->curr` handoff).
static PREV_TASK: RacyCell<*mut TaskStruct> = RacyCell::new(core::ptr::null_mut());

/// Total context switches, for reporting (`nr_context_switches`).
static NR_SWITCHES: AtomicU64 = AtomicU64::new(0);

/// `set_tsk_need_resched()`.
pub fn set_need_resched() {
    NEED_RESCHED.store(true, Ordering::Relaxed);
}

/// `need_resched()`.
pub fn need_resched() -> bool {
    NEED_RESCHED.load(Ordering::Relaxed)
}

/// `sched_init()`: turn the boot context into the idle task `swapper/0`.
pub fn sched_init(idle: *mut TaskStruct) {
    // SAFETY: the runqueue head is a static that stays pinned, and idle is
    // the boot CPU's static idle task, not yet shared.
    unsafe {
        RQ.lock_irqsave().queue.init();
        (*idle).pid = 0;
        (*idle).flags = PF_IDLE | PF_KTHREAD;
        (*idle).state.store(TASK_RUNNING, Ordering::Relaxed);
        let comm = &mut (*idle).comm;
        comm[..9].copy_from_slice(b"swapper/0");
        (*idle).run_list.init();
    }
    unsafe extern "C" {
        static boot_stack_end: u8;
    }
    // SAFETY: idle runs on the boot stack; record its top for the TSS.
    unsafe { (*idle).kstack_top = &raw const boot_stack_end as u64 };
    IDLE_TASK.store(idle, Ordering::Release);
    set_current(idle);
}

/// `cpu_idle()` / `do_idle()`: the idle task's loop. Reschedule when a task
/// becomes runnable, otherwise halt until the next interrupt.
pub fn cpu_idle_loop() -> ! {
    use crate::arch::x86::include::irqflags::native_safe_halt;
    loop {
        while need_resched() {
            schedule();
        }
        native_safe_halt();
    }
}

/// `enqueue_task()`: put a runnable task at the tail of the runqueue.
fn enqueue_task(rq: &mut Rq, task: *mut TaskStruct) {
    // SAFETY: task is a valid, pinned TaskStruct not currently queued.
    unsafe { list_add_tail(&mut (*task).run_list, &mut rq.queue) };
    rq.nr_running += 1;
}

/// `dequeue_task()`.
fn dequeue_task(rq: &mut Rq, task: *mut TaskStruct) {
    // SAFETY: task is currently on the runqueue.
    unsafe { list_del_init(&mut (*task).run_list) };
    rq.nr_running -= 1;
}

/// `wake_up_new_task()`: make a freshly created task runnable.
pub fn wake_up_new_task(task: *mut TaskStruct) {
    // SAFETY: task is a valid new TaskStruct.
    unsafe {
        (*task).time_slice = rr_timeslice();
        (*task).state.store(TASK_RUNNING, Ordering::Release);
    }
    enqueue_task(&mut RQ.lock_irqsave(), task);
    set_need_resched();
}

/// `pick_next_task()`: the runqueue head, or the idle task if empty.
fn pick_next_task(rq: &Rq) -> *mut TaskStruct {
    // SAFETY: a non-empty queue's head links a task's run_list.
    unsafe {
        if rq.nr_running == 0 {
            return IDLE_TASK.load(Ordering::Acquire);
        }
        let head = &rq.queue as *const ListHead as *mut ListHead;
        list_entry::<TaskStruct>((*head).next, RUN_LIST_OFFSET)
    }
}

/// `context_switch()`: switch the CPU from `prev` to `next`.
fn context_switch(prev: *mut TaskStruct, next: *mut TaskStruct) {
    NR_SWITCHES.fetch_add(1, Ordering::Relaxed);
    // Point the CPU at `next`'s kernel stack for traps and syscalls from
    // user mode, and switch to its address space if it is a user task.
    switch_mm_and_stack(next);
    // Hand `prev` to the resumed task's finish step (schedule_tail /
    // finish_task_switch), which may free a dead task.
    // SAFETY: single CPU, interrupts off; PREV_TASK is only touched here and
    // in the finish step.
    unsafe { *PREV_TASK.get() = prev };
    set_current(next);
    // Kernel threads share the kernel page tables, so there is no mm to
    // switch. Swap the kernel stack and callee-saved registers.
    // SAFETY: both tasks are valid; thread is at offset 0 so &mut task.thread.sp
    // is the saved-SP slot the asm expects.
    unsafe {
        switch_to_asm(&mut (*prev).thread.sp, &mut (*next).thread.sp);
    }
    // Execution resumes here when `prev` is later scheduled back in.
    finish_task_switch();
}

/// Updates the TSS `sp0`, the SYSCALL stack and, for user tasks, CR3 to the
/// task being switched to (`switch_mm` + `switch_to`'s stack handling).
fn switch_mm_and_stack(next: *mut TaskStruct) {
    use crate::arch::x86::kernel::cpu::common::set_tss_sp0;
    use crate::arch::x86::kernel::syscall_64::KERNEL_STACK_TOP;
    use crate::arch::x86::mm::init::kernel_pgd;
    use crate::arch::x86::mm::user::load_cr3;
    // SAFETY: next is a valid task; these globals are per-CPU on one CPU.
    unsafe {
        let top = (*next).kstack_top;
        if top != 0 {
            set_tss_sp0(top);
            *KERNEL_STACK_TOP.get() = top;
        }
        let pgd = if (*next).mm_pgd != 0 {
            (*next).mm_pgd
        } else {
            kernel_pgd()
        };
        load_cr3(pgd);
    }
}

/// `finish_task_switch()`: bookkeeping after a switch, in the context of
/// the task that was just switched *to*. Reaps a task that exited.
fn finish_task_switch() {
    // SAFETY: PREV_TASK was set by the switching code before the switch.
    let prev = unsafe { *PREV_TASK.get() };
    if prev.is_null() {
        return;
    }
    // SAFETY: prev is a valid task that just stopped running.
    if unsafe { (*prev).state.load(Ordering::Acquire) } == TASK_DEAD {
        reap_task(prev);
    }
}

/// `schedule_tail()`: the finish step run by a brand-new task before it
/// starts its thread function.
pub fn schedule_tail() {
    finish_task_switch();
}

/// `__schedule()`: pick the next task and switch to it. Must be called with
/// interrupts disabled; returns (when this task is next scheduled) with
/// interrupts still disabled.
fn __schedule(preempt: bool) {
    NEED_RESCHED.store(false, Ordering::Relaxed);
    let prev = current();

    let next = {
        let mut rq = RQ.lock_irqsave();
        // SAFETY: prev is the running task.
        let prev_state = unsafe { (*prev).state.load(Ordering::Acquire) };
        let prev_idle = unsafe { (*prev).flags & PF_IDLE != 0 };

        if prev_state == TASK_RUNNING && !prev_idle {
            // Still runnable: rotate to the tail for round-robin. (A task
            // that blocked or exited was already dequeued by the code that
            // changed its state.)
            // SAFETY: prev is on the runqueue.
            unsafe {
                list_del_init(&mut (*prev).run_list);
                list_add_tail(&mut (*prev).run_list, &mut rq.queue);
            }
        }
        pick_next_task(&rq)
    };

    if next == prev {
        return;
    }

    // SAFETY: both are valid tasks.
    unsafe {
        if preempt {
            (*prev).nivcsw += 1;
        } else {
            (*prev).nvcsw += 1;
        }
    }
    context_switch(prev, next);
}

/// `schedule()`: voluntarily yield the CPU.
pub fn schedule() {
    let flags = local_irq_save();
    __schedule(false);
    local_irq_restore(flags);
}

/// Called from the timer-IRQ return path: preempt if a reschedule is due
/// and we are not in a non-preemptible section. Interrupts are already off.
pub fn preempt_on_irq_return() {
    if need_resched() && crate::kernel::softirq::preempt_count() == 0 && !in_interrupt() {
        __schedule(true);
    }
}

/// `sched_tick()`: charge the running task one tick and preempt it at the
/// end of its slice.
pub fn scheduler_tick() {
    let curr = current();
    // SAFETY: curr is the running task.
    unsafe {
        if (*curr).flags & PF_IDLE != 0 {
            // The idle task yields as soon as anything is runnable.
            if RQ.lock_irqsave().nr_running != 0 {
                set_need_resched();
            }
            return;
        }
        if (*curr).time_slice > 0 {
            (*curr).time_slice -= 1;
        }
        if (*curr).time_slice == 0 {
            (*curr).time_slice = rr_timeslice();
            set_need_resched();
        }
    }
}

/// `do_exit()`: the current task is done. Mark it dead, dequeue it, and
/// switch away forever; the next task reaps its stack.
pub fn do_exit(_code: i32) -> ! {
    let task = current();
    local_irq_save();
    // SAFETY: task is the running task.
    unsafe {
        (*task).state.store(TASK_DEAD, Ordering::Release);
        dequeue_task(&mut RQ.lock_irqsave(), task);
    }
    __schedule(false);
    // The scheduler never returns to a dead task.
    unreachable!("scheduled a dead task");
}

/// Frees a dead task: its stack now, and the descriptor. Runs in another
/// task's context, so it is safe to free the dead task's stack.
fn reap_task(task: *mut TaskStruct) {
    // SAFETY: task is dead and no longer running or queued.
    unsafe {
        free_thread_stack(&*task);
        (*task).state.store(TASK_NEW, Ordering::Release);
        crate::mm::slub::kfree(task as u64);
    }
}

/// Spawns a kernel thread running `func(arg)`; see `kernel_thread()`.
///
/// Returns the task, or null on failure.
pub fn kernel_thread(func: extern "C" fn(u64) -> i32, arg: u64, name: &[u8]) -> *mut TaskStruct {
    use crate::mm::slub::kmalloc;
    let task = kmalloc(core::mem::size_of::<TaskStruct>(), __GFP_ZERO) as *mut TaskStruct;
    if task.is_null() {
        return core::ptr::null_mut();
    }
    // SAFETY: task is freshly allocated and sized for a TaskStruct.
    unsafe {
        task.write(TaskStruct::new());
        (*task).flags = PF_KTHREAD;
        (*task).pid = alloc_pid();
        let n = name.len().min(TASK_COMM_LEN - 1);
        let comm = &mut (*task).comm;
        comm[..n].copy_from_slice(&name[..n]);
        if !copy_thread(&mut *task, func, arg) {
            crate::mm::slub::kfree(task as u64);
            return core::ptr::null_mut();
        }
    }
    wake_up_new_task(task);
    task
}

/// Allocates a PID (`alloc_pid()`), monotonically for now.
fn alloc_pid() -> i32 {
    static NEXT_PID: AtomicU64 = AtomicU64::new(1);
    NEXT_PID.fetch_add(1, Ordering::Relaxed) as i32
}

/// `nr_context_switches()`.
pub fn nr_context_switches() -> u64 {
    NR_SWITCHES.load(Ordering::Relaxed)
}

/// Iterates the runnable tasks' pids, for reporting.
pub fn for_each_running_pid(mut f: impl FnMut(i32, &[u8])) {
    let rq = RQ.lock_irqsave();
    // SAFETY: the queue links live tasks while the lock is held.
    unsafe {
        for node in iter(&rq.queue as *const ListHead as *mut ListHead) {
            let task = list_entry::<TaskStruct>(node, RUN_LIST_OFFSET);
            f((*task).pid, (*task).name());
        }
    }
}

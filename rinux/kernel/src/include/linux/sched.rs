// SPDX-License-Identifier: GPL-2.0
//! Tasks.
//!
//! A partial port of `struct task_struct` and `struct thread_struct` from
//! `include/linux/sched.h` and `arch/x86/include/asm/processor.h`, with the
//! `current` accessor. rinux has only kernel threads so far, so the fields
//! are the ones the scheduler and the context switch need; mm, files,
//! signals, credentials, cgroups and the rest come with user space.

use core::sync::atomic::{AtomicPtr, AtomicU32, Ordering};

use rinux_lib::list::ListHead;

use crate::arch::x86::include::page::PAGE_SHIFT;

/// `THREAD_SIZE`: the kernel stack is 16 KiB (`THREAD_SIZE_ORDER` = 2).
pub const THREAD_SIZE_ORDER: u32 = 2;
pub const THREAD_SIZE: usize = (1 << PAGE_SHIFT) << THREAD_SIZE_ORDER;

// Task states (`task_struct::__state`).
pub const TASK_RUNNING: u32 = 0x0000;
pub const TASK_INTERRUPTIBLE: u32 = 0x0001;
pub const TASK_UNINTERRUPTIBLE: u32 = 0x0002;
pub const TASK_DEAD: u32 = 0x0080;
pub const TASK_NEW: u32 = 0x0800;

// Per-task flags (`task_struct::flags`).
pub const PF_IDLE: u32 = 0x0000_0002;
pub const PF_KTHREAD: u32 = 0x0020_0000;

/// Length of `comm`.
pub const TASK_COMM_LEN: usize = 16;

/// `struct thread_struct`: the CPU state saved across a context switch.
pub struct ThreadStruct {
    /// Saved kernel stack pointer (`thread.sp`); the switch reads and writes
    /// it. The initial stack frame makes a new thread start at the kthread
    /// trampoline.
    pub sp: u64,
}

/// `struct task_struct`.
#[repr(C)]
pub struct TaskStruct {
    /// Saved registers; `offset_of` of this must stay 0 so the context
    /// switch can take `&mut task.thread.sp` without a struct offset.
    pub thread: ThreadStruct,
    /// `__state`.
    pub state: AtomicU32,
    pub flags: u32,
    pub pid: i32,
    pub comm: [u8; TASK_COMM_LEN],
    /// Runqueue linkage (`task_struct::run_list` of the RT scheduler).
    pub run_list: ListHead,
    /// Base of the kernel stack allocation, and its page order.
    pub stack: u64,
    pub stack_order: u32,
    /// Top of the kernel stack (for the TSS `sp0` and the SYSCALL stub).
    pub kstack_top: u64,
    /// User address space (`mm->pgd` physical); 0 for a kernel thread.
    pub mm_pgd: u64,
    /// Remaining ticks in this round-robin slice (`rt.time_slice`).
    pub time_slice: u32,
    /// Voluntary and involuntary context-switch counts (`nvcsw`/`nivcsw`).
    pub nvcsw: u64,
    pub nivcsw: u64,
}

impl TaskStruct {
    /// An empty task; fields are filled in by `copy_process`.
    pub const fn new() -> TaskStruct {
        TaskStruct {
            thread: ThreadStruct { sp: 0 },
            state: AtomicU32::new(TASK_NEW),
            flags: 0,
            pid: 0,
            comm: [0; TASK_COMM_LEN],
            run_list: ListHead::new(),
            stack: 0,
            stack_order: 0,
            kstack_top: 0,
            mm_pgd: 0,
            time_slice: 0,
            nvcsw: 0,
            nivcsw: 0,
        }
    }

    /// `__set_current_state()` style state store.
    pub fn set_state(&self, state: u32) {
        self.state.store(state, Ordering::Release);
    }

    /// The name as a byte string, without the trailing NUL.
    pub fn name(&self) -> &[u8] {
        &self.comm[..rinux_lib::string::strlen(&self.comm)]
    }
}

impl Default for TaskStruct {
    fn default() -> TaskStruct {
        TaskStruct::new()
    }
}

// The run_list links tasks owned by the runqueue; access is serialized by
// the runqueue lock.
const _: () = assert!(core::mem::offset_of!(TaskStruct, thread) == 0);

/// The currently running task on the (single) CPU.
static CURRENT: AtomicPtr<TaskStruct> = AtomicPtr::new(core::ptr::null_mut());

/// `current`.
#[inline]
pub fn current() -> *mut TaskStruct {
    CURRENT.load(Ordering::Acquire)
}

/// Sets `current` (called by the context switch).
#[inline]
pub fn set_current(task: *mut TaskStruct) {
    CURRENT.store(task, Ordering::Release);
}

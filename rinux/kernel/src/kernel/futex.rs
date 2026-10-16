// SPDX-License-Identifier: GPL-2.0-or-later
//! Fast user-space mutexes.
//!
//! A port of the core of `kernel/futex/core.c`, `waitwake.c` and
//! `syscalls.c`: `FUTEX_WAIT` checks that the futex word still holds the
//! value the caller saw, queues the task as a waiter on that address and
//! sleeps until a `FUTEX_WAKE` on the same address or its timeout;
//! `FUTEX_WAKE` wakes up to `nr` waiters and reports how many. Also the
//! exit-time `exit_robust_list()`/`handle_futex_death()` walk that releases
//! robust locks a dying task still held.
//!
//! Simplified from Linux's: one waiter list instead of the hashed
//! `futex_hash_bucket`s, keys are plain user addresses (a single address
//! space, so no `futex_key` of mm/inode), and the wait is a polled sleep with
//! interrupts on. rinux has one task, so a wake can only ever come from a
//! later task: a `FUTEX_WAIT` with no timeout here blocks for good, exactly as
//! the same deadlock would in Linux. No PI futexes, requeue, bitsets or
//! `FUTEX_WAKE_OP`.

use alloc::vec::Vec;
use core::sync::atomic::{AtomicU64, Ordering};

use crate::arch::x86::include::irqflags::{local_irq_disable, native_safe_halt};
use crate::kernel::locking::SpinLock;
use crate::kernel::time::jiffies::jiffies;

// `include/uapi/linux/futex.h`.
pub const FUTEX_WAIT: u32 = 0;
pub const FUTEX_WAKE: u32 = 1;
pub const FUTEX_PRIVATE_FLAG: u32 = 128;
pub const FUTEX_CLOCK_REALTIME: u32 = 256;
/// `FUTEX_CMD_MASK`: the operation with its flag bits stripped.
pub const FUTEX_CMD_MASK: u32 = !(FUTEX_PRIVATE_FLAG | FUTEX_CLOCK_REALTIME);
/// Bits of a robust/PI futex word: waiters present, the owner died, and the
/// owner's tid.
pub const FUTEX_WAITERS: u32 = 0x8000_0000;
pub const FUTEX_OWNER_DIED: u32 = 0x4000_0000;
pub const FUTEX_TID_MASK: u32 = 0x3fff_ffff;
/// `ROBUST_LIST_LIMIT`: the most entries an exit walk follows (a cycle guard).
pub const ROBUST_LIST_LIMIT: usize = 2048;

const EAGAIN: i64 = -11;
const ETIMEDOUT: i64 = -110;

/// `struct futex_q`: a task blocked in `FUTEX_WAIT` on `uaddr`. `woken` is
/// set by `FUTEX_WAKE`; `token` lets the sleeper find its own entry.
struct FutexQ {
    uaddr: u64,
    token: u64,
    woken: bool,
}

/// The waiter list (what the hash buckets hold, collapsed to one).
static FUTEX_QUEUES: SpinLock<Vec<FutexQ>> = SpinLock::new(Vec::new());
static NEXT_TOKEN: AtomicU64 = AtomicU64::new(1);

/// `futex_wait()`: if the word at `uaddr` still equals `val`, sleep until
/// woken (0) or until `timeout_ticks` jiffies pass (`-ETIMEDOUT`). Returns
/// `-EAGAIN` if the word has already changed.
pub fn futex_wait(uaddr: u64, val: u32, timeout_ticks: Option<u64>) -> i64 {
    // futex_wait_setup(): the value check and the queueing are one step from
    // the waker's point of view, so take the queue lock around both.
    let token = NEXT_TOKEN.fetch_add(1, Ordering::Relaxed);
    {
        let mut q = FUTEX_QUEUES.lock();
        // SAFETY: syscall context on the user's page tables; a bad address
        // faults and kills the task, as get_user() would fail it.
        let cur = unsafe { (uaddr as *const u32).read_volatile() };
        if cur != val {
            return EAGAIN;
        }
        q.push(FutexQ {
            uaddr,
            token,
            woken: false,
        });
    }
    let deadline = timeout_ticks.map(|t| jiffies() + t);

    // The SYSCALL entry masked interrupts; sleep with them on so the tick (and
    // a wake from another context) can arrive, then restore the entry state.
    let ret = loop {
        {
            let mut q = FUTEX_QUEUES.lock();
            match q.iter().position(|w| w.token == token) {
                Some(pos) if q[pos].woken => {
                    q.remove(pos);
                    break 0;
                }
                Some(_) => {}
                None => break 0,
            }
            if deadline.is_some_and(|d| jiffies() >= d) {
                if let Some(pos) = q.iter().position(|w| w.token == token) {
                    q.remove(pos);
                }
                break ETIMEDOUT;
            }
        }
        native_safe_halt();
    };
    local_irq_disable();
    ret
}

/// `futex_wake()`: wake up to `nr` waiters on `uaddr`, returning how many.
pub fn futex_wake(uaddr: u64, nr: u32) -> i64 {
    let mut q = FUTEX_QUEUES.lock();
    let mut woken = 0u32;
    for w in q.iter_mut() {
        if woken >= nr {
            break;
        }
        if w.uaddr == uaddr && !w.woken {
            w.woken = true;
            woken += 1;
        }
    }
    woken as i64
}

/// `handle_futex_death()`: if the lock at `uaddr` is owned by `pid`, mark it
/// `FUTEX_OWNER_DIED` (keeping the waiters bit) and wake one waiter. Returns
/// whether the lock was held.
fn handle_futex_death(uaddr: u64, pid: u32) -> bool {
    // SAFETY: syscall context on the dying task's page tables; the address
    // came from its own robust list.
    unsafe {
        let p = uaddr as *mut u32;
        let val = p.read_volatile();
        if val & FUTEX_TID_MASK != pid {
            return false;
        }
        p.write_volatile((val & FUTEX_WAITERS) | FUTEX_OWNER_DIED);
        if val & FUTEX_WAITERS != 0 {
            futex_wake(uaddr, 1);
        }
        true
    }
}

/// `exit_robust_list()`: walk the dying task's robust-futex list (head at
/// `head`: `list.next`\@0, `futex_offset`\@8, `list_op_pending`\@16; each
/// entry's `next` at offset 0, its lock word at `entry + futex_offset`),
/// releasing every lock it still owned. Returns the number released.
pub fn exit_robust_list(head: u64, pid: u32) -> usize {
    if head == 0 {
        return 0;
    }
    let mut released = 0;
    // SAFETY: as handle_futex_death; a corrupt list is bounded by
    // ROBUST_LIST_LIMIT and a bad pointer faults.
    unsafe {
        let futex_offset = ((head + 8) as *const i64).read_volatile();
        let pending = ((head + 16) as *const u64).read_volatile();
        let mut entry = (head as *const u64).read_volatile();
        let mut limit = ROBUST_LIST_LIMIT;
        while entry != head && limit > 0 {
            limit -= 1;
            let next = (entry as *const u64).read_volatile();
            // The lock being taken when the task died is handled last.
            if entry != pending && handle_futex_death(entry.wrapping_add(futex_offset as u64), pid)
            {
                released += 1;
            }
            entry = next;
        }
        if pending != 0 && handle_futex_death(pending.wrapping_add(futex_offset as u64), pid) {
            released += 1;
        }
    }
    released
}

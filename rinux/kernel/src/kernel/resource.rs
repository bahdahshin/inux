// SPDX-License-Identifier: GPL-2.0
//! Resource limits.
//!
//! A port of `include/uapi/asm-generic/resource.h`,
//! `include/uapi/linux/resource.h` and the `INIT_RLIMITS` table of
//! `include/asm-generic/resource.h`: the per-process limits that
//! `prlimit64`/`getrlimit`/`setrlimit` read and write (`do_prlimit()` in
//! `kernel/sys.c`).
//!
//! Simplified: rinux runs one task, so the limits live in one global table
//! (what `signal_struct->rlim` holds per process) rather than per task, and
//! none of them are enforced yet -- they are state user space can query and
//! set, which a libc does at startup (e.g. `RLIMIT_STACK`, `RLIMIT_NOFILE`).

use crate::kernel::locking::SpinLock;

pub const RLIMIT_CPU: u32 = 0;
pub const RLIMIT_FSIZE: u32 = 1;
pub const RLIMIT_DATA: u32 = 2;
pub const RLIMIT_STACK: u32 = 3;
pub const RLIMIT_CORE: u32 = 4;
pub const RLIMIT_RSS: u32 = 5;
pub const RLIMIT_NPROC: u32 = 6;
pub const RLIMIT_NOFILE: u32 = 7;
pub const RLIMIT_MEMLOCK: u32 = 8;
pub const RLIMIT_AS: u32 = 9;
pub const RLIMIT_LOCKS: u32 = 10;
pub const RLIMIT_SIGPENDING: u32 = 11;
pub const RLIMIT_MSGQUEUE: u32 = 12;
pub const RLIMIT_NICE: u32 = 13;
pub const RLIMIT_RTPRIO: u32 = 14;
pub const RLIMIT_RTTIME: u32 = 15;
/// `RLIM_NLIMITS`: how many resources there are.
pub const RLIM_NLIMITS: usize = 16;

/// `RLIM_INFINITY` / `RLIM64_INFINITY`: no limit.
pub const RLIM_INFINITY: u64 = !0;
/// `_STK_LIM`: the default soft stack limit, 8 MiB.
pub const STK_LIM: u64 = 8 * 1024 * 1024;
/// `MLOCK_LIMIT`: the default locked-memory limit, 8 MiB.
pub const MLOCK_LIMIT: u64 = 8 * 1024 * 1024;
/// `INR_OPEN_CUR` / `INR_OPEN_MAX`: default soft and hard open-file limits.
pub const INR_OPEN_CUR: u64 = 1024;
pub const INR_OPEN_MAX: u64 = 4096;
/// `MQ_BYTES_MAX`: the default POSIX message-queue byte limit.
pub const MQ_BYTES_MAX: u64 = 819_200;

/// `struct rlimit64`: a soft (current) and hard (maximum) limit.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Rlimit {
    pub rlim_cur: u64,
    pub rlim_max: u64,
}

const fn lim(cur: u64, max: u64) -> Rlimit {
    Rlimit {
        rlim_cur: cur,
        rlim_max: max,
    }
}

/// `INIT_RLIMITS`: the limits the first task starts with, indexed by
/// `RLIMIT_*`.
pub const INIT_RLIMITS: [Rlimit; RLIM_NLIMITS] = [
    lim(RLIM_INFINITY, RLIM_INFINITY), // RLIMIT_CPU
    lim(RLIM_INFINITY, RLIM_INFINITY), // RLIMIT_FSIZE
    lim(RLIM_INFINITY, RLIM_INFINITY), // RLIMIT_DATA
    lim(STK_LIM, RLIM_INFINITY),       // RLIMIT_STACK
    lim(0, RLIM_INFINITY),             // RLIMIT_CORE
    lim(RLIM_INFINITY, RLIM_INFINITY), // RLIMIT_RSS
    lim(0, 0),                         // RLIMIT_NPROC
    lim(INR_OPEN_CUR, INR_OPEN_MAX),   // RLIMIT_NOFILE
    lim(MLOCK_LIMIT, MLOCK_LIMIT),     // RLIMIT_MEMLOCK
    lim(RLIM_INFINITY, RLIM_INFINITY), // RLIMIT_AS
    lim(RLIM_INFINITY, RLIM_INFINITY), // RLIMIT_LOCKS
    lim(0, 0),                         // RLIMIT_SIGPENDING
    lim(MQ_BYTES_MAX, MQ_BYTES_MAX),   // RLIMIT_MSGQUEUE
    lim(0, 0),                         // RLIMIT_NICE
    lim(0, 0),                         // RLIMIT_RTPRIO
    lim(RLIM_INFINITY, RLIM_INFINITY), // RLIMIT_RTTIME
];

/// `signal_struct->rlim`: the (single) process's current limits.
pub static RLIMITS: SpinLock<[Rlimit; RLIM_NLIMITS]> = SpinLock::new(INIT_RLIMITS);

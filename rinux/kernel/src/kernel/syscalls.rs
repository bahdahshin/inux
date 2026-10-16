// SPDX-License-Identifier: GPL-2.0
//! System calls.
//!
//! A small port of the x86-64 syscall table (`arch/x86/entry/syscalls/`) and
//! the handlers they reach in `fs/read_write.c`, `kernel/exit.c` and
//! `kernel/sys.c`. Only what a tiny static init needs is implemented; the
//! rest return `-ENOSYS`.
//!
//! The syscall runs on the user task's page tables (which also map the
//! kernel), so user pointers can be read and written directly. The accesses
//! are unchecked for now: a bad pointer faults, and the page-fault handler
//! kills the user task rather than returning an error, which the user-copy
//! helpers of `arch/x86/lib/` do more gracefully.

use crate::arch::x86::include::ptrace::PtRegs;
use crate::include::linux::sched::current;

// x86-64 syscall numbers (`unistd_64.h`).
const SYS_READ: u64 = 0;
const SYS_WRITE: u64 = 1;
const SYS_CLOSE: u64 = 3;
const SYS_BRK: u64 = 12;
const SYS_WRITEV: u64 = 20;
const SYS_GETPID: u64 = 39;
const SYS_UNAME: u64 = 63;
const SYS_EXIT: u64 = 60;
const SYS_EXIT_GROUP: u64 = 231;

const SYS_OPEN: u64 = 2;
const SYS_MMAP: u64 = 9;
const SYS_MPROTECT: u64 = 10;
const SYS_MUNMAP: u64 = 11;
const SYS_STAT: u64 = 4;
const SYS_LSEEK: u64 = 8;
const SYS_NANOSLEEP: u64 = 35;
const SYS_GETTIMEOFDAY: u64 = 96;
const SYS_RT_SIGACTION: u64 = 13;
const SYS_RT_SIGPROCMASK: u64 = 14;
const SYS_RT_SIGRETURN: u64 = 15;
const SYS_KILL: u64 = 62;
const SYS_GETCWD: u64 = 79;
const SYS_SYSINFO: u64 = 99;
const SYS_GETUID: u64 = 102;
const SYS_GETPPID: u64 = 110;
const SYS_ARCH_PRCTL: u64 = 158;
const SYS_FUTEX: u64 = 202;
const SYS_SET_TID_ADDRESS: u64 = 218;
const SYS_SET_ROBUST_LIST: u64 = 273;
const SYS_PRLIMIT64: u64 = 302;
const SYS_GETGID: u64 = 104;
const SYS_GETEUID: u64 = 107;
const SYS_GETEGID: u64 = 108;
const SYS_CAPGET: u64 = 125;
const SYS_GETDENTS64: u64 = 217;
const SYS_CLOCK_GETTIME: u64 = 228;
const SYS_GETRANDOM: u64 = 318;

/// `getrandom(2)` flags (`include/uapi/linux/random.h`). rinux's pool never
/// blocks, so these only gate which flags are accepted.
const GRND_NONBLOCK: u64 = 0x0001;
const GRND_RANDOM: u64 = 0x0002;
const GRND_INSECURE: u64 = 0x0004;

const ENOSYS: i64 = -38;
const EBADF: i64 = -9;
const EMFILE: i64 = -24;
const EINVAL: i64 = -22;
const EFAULT: i64 = -14;
const ERANGE: i64 = -34;
const ESRCH: i64 = -3;

/// `getcap`/`capset` ABI versions (`uapi/linux/capability.h`).
const LINUX_CAPABILITY_VERSION_1: u32 = 0x1998_0330;
const LINUX_CAPABILITY_VERSION_2: u32 = 0x2007_1026;
const LINUX_CAPABILITY_VERSION_3: u32 = 0x2008_0522;

/// `CLOCK_REALTIME` / `CLOCK_MONOTONIC` clock ids (`include/uapi/linux/time.h`).
const CLOCK_REALTIME: u64 = 0;
const CLOCK_MONOTONIC: u64 = 1;

/// User file descriptors start after the three console descriptors.
const FD_BASE: u64 = 3;
/// A tiny open-file table. rinux runs one user process at a time, so a single
/// global table stands in for a per-task `files_struct`.
const FD_MAX: usize = 16;
static FD_TABLE: crate::kernel::locking::SpinLock<[Option<crate::fs::vfs::File>; FD_MAX]> =
    crate::kernel::locking::SpinLock::new([const { None }; FD_MAX]);

/// Where a user `exit()` leaves its code, for the launcher to report.
pub static USER_EXIT_CODE: core::sync::atomic::AtomicI64 =
    core::sync::atomic::AtomicI64::new(i64::MIN);

/// `task->clear_child_tid`: the user address `set_tid_address()` registered,
/// which the exit path zeroes and futex-wakes in Linux (`mm_release()`). One
/// task, so one global; the exit-time clearing is not wired yet.
static CLEAR_CHILD_TID: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);
/// `task->robust_list`: the head of the thread's robust-futex list, walked on
/// exit to release locks a dying thread held. Recorded only, for now.
static ROBUST_LIST_HEAD: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);

/// Copies `len` bytes from user address `uaddr` into a kernel `Vec`.
///
/// # Safety
///
/// Runs on the user task's page tables; `uaddr` is read directly and a bad
/// address faults (killing the task).
unsafe fn copy_from_user(uaddr: u64, len: usize) -> alloc::vec::Vec<u8> {
    let mut v = alloc::vec::Vec::with_capacity(len);
    for i in 0..len {
        // SAFETY: see the function contract.
        v.push(unsafe { (uaddr as *const u8).add(i).read_volatile() });
    }
    v
}

/// `syscall_dispatch()`: run the syscall `nr` with arguments from `regs`.
pub fn syscall_dispatch(nr: u64, regs: &mut PtRegs) -> i64 {
    match nr {
        SYS_WRITE => sys_write(regs.di, regs.si, regs.dx as usize),
        SYS_WRITEV => sys_writev(regs.di, regs.si, regs.dx as usize),
        SYS_READ => sys_read(regs.di, regs.si, regs.dx as usize),
        SYS_GETPID => sys_getpid(),
        SYS_UNAME => sys_uname(regs.di),
        SYS_BRK => sys_brk(regs.di),
        SYS_OPEN => sys_open(regs.di, regs.si, regs.dx),
        SYS_MMAP => sys_mmap(regs.di, regs.si, regs.dx, regs.r10, regs.r8, regs.r9),
        SYS_MPROTECT => crate::mm::mmap::do_mprotect(regs.di, regs.si, regs.dx),
        SYS_MUNMAP => crate::mm::mmap::do_munmap(regs.di, regs.si),
        SYS_STAT => sys_stat(regs.di, regs.si),
        SYS_LSEEK => sys_lseek(regs.di, regs.si as i64, regs.dx as u32),
        SYS_GETDENTS64 => sys_getdents64(regs.di, regs.si, regs.dx as usize),
        SYS_CLOCK_GETTIME => sys_clock_gettime(regs.di, regs.si),
        SYS_GETTIMEOFDAY => sys_gettimeofday(regs.di, regs.si),
        SYS_NANOSLEEP => sys_nanosleep(regs.di, regs.si),
        SYS_GETRANDOM => sys_getrandom(regs.di, regs.si as usize, regs.dx),
        SYS_SYSINFO => sys_sysinfo(regs.di),
        SYS_RT_SIGACTION => sys_rt_sigaction(regs.di, regs.si, regs.dx, regs.r10),
        SYS_RT_SIGPROCMASK => sys_rt_sigprocmask(regs.di, regs.si, regs.dx, regs.r10),
        SYS_RT_SIGRETURN => crate::arch::x86::kernel::signal::sys_rt_sigreturn(regs),
        SYS_KILL => sys_kill(regs.di as i64, regs.si),
        SYS_GETCWD => sys_getcwd(regs.di, regs.si),
        SYS_ARCH_PRCTL => sys_arch_prctl(regs.di, regs.si),
        SYS_GETPPID => sys_getppid(),
        SYS_SET_TID_ADDRESS => sys_set_tid_address(regs.di),
        SYS_SET_ROBUST_LIST => sys_set_robust_list(regs.di, regs.si),
        SYS_PRLIMIT64 => sys_prlimit64(regs.di as i64, regs.si, regs.dx, regs.r10),
        SYS_FUTEX => sys_futex(regs.di, regs.si, regs.dx, regs.r10, regs.r8, regs.r9),
        SYS_GETUID => crate::kernel::cred::current_cred().uid as i64,
        SYS_GETGID => crate::kernel::cred::current_cred().gid as i64,
        SYS_GETEUID => crate::kernel::cred::current_cred().euid as i64,
        SYS_GETEGID => crate::kernel::cred::current_cred().egid as i64,
        SYS_CAPGET => sys_capget(regs.di, regs.si),
        SYS_CLOSE => sys_close(regs.di),
        SYS_EXIT | SYS_EXIT_GROUP => sys_exit(regs.di as i32),
        _ => {
            pr_warn!("syscall: unimplemented nr {} from pid {}", nr, unsafe {
                (*current()).pid
            });
            ENOSYS
        }
    }
}

/// `sys_write()`: fds 1 and 2 go to the console.
fn sys_write(fd: u64, buf: u64, count: usize) -> i64 {
    if fd != 1 && fd != 2 {
        return EBADF;
    }
    if count == 0 {
        return 0;
    }
    // SAFETY: syscall context; a bad buffer faults and kills the task.
    let data = unsafe { copy_from_user(buf, count) };
    crate::kernel::printk::console_write_raw(&data);
    count as i64
}

/// `sys_writev()`: minimal, just the console fds.
fn sys_writev(fd: u64, iov: u64, iovcnt: usize) -> i64 {
    if fd != 1 && fd != 2 {
        return EBADF;
    }
    let mut total = 0i64;
    for i in 0..iovcnt {
        // struct iovec { void *base; size_t len; }
        let base = iov + (i as u64) * 16;
        // SAFETY: syscall context.
        let (ptr, len) = unsafe {
            (
                (base as *const u64).read_volatile(),
                (base as *const u64).add(1).read_volatile() as usize,
            )
        };
        if len == 0 {
            continue;
        }
        // SAFETY: syscall context.
        let data = unsafe { copy_from_user(ptr, len) };
        crate::kernel::printk::console_write_raw(&data);
        total += len as i64;
    }
    total
}

/// `sys_read()`: fd 0 (stdin) has no input source, so it is at EOF; an open
/// file descriptor reads through the VFS into the user buffer.
fn sys_read(fd: u64, buf: u64, count: usize) -> i64 {
    if fd == 0 {
        return 0; // stdin: end of file
    }
    if fd < FD_BASE {
        return EBADF; // stdout/stderr are not readable
    }
    let idx = (fd - FD_BASE) as usize;
    let mut tbl = FD_TABLE.lock();
    let file = match tbl.get_mut(idx).and_then(|o| o.as_mut()) {
        Some(f) => f,
        None => return EBADF,
    };
    let want = count.min(1 << 16);
    let mut kbuf = alloc::vec![0u8; want];
    let n = match crate::fs::vfs::vfs_read(file, &mut kbuf) {
        Ok(n) => n,
        Err(e) => return e.to_errno() as i64,
    };
    // SAFETY: syscall context on the user's page tables; a bad buffer faults.
    unsafe {
        for (i, &b) in kbuf.iter().take(n).enumerate() {
            (buf as *mut u8).add(i).write_volatile(b);
        }
    }
    n as i64
}

/// `sys_open()`: open `path` (a user C string) through the VFS and install it
/// in the open-file table, returning the new descriptor.
fn sys_open(path_ptr: u64, flags: u64, _mode: u64) -> i64 {
    // SAFETY: syscall context; the path is a user pointer read until NUL.
    let path = unsafe { copy_user_cstr(path_ptr, 256) };
    match crate::fs::vfs::filp_open(&path, flags as u32, 0o644) {
        Ok(file) => {
            // alloc_fd(): a descriptor number at or beyond RLIMIT_NOFILE's
            // soft limit is refused with EMFILE.
            let nofile = crate::kernel::resource::RLIMITS.lock()
                [crate::kernel::resource::RLIMIT_NOFILE as usize]
                .rlim_cur;
            let mut tbl = FD_TABLE.lock();
            for (i, slot) in tbl.iter_mut().enumerate() {
                if slot.is_none() {
                    if FD_BASE + i as u64 >= nofile {
                        return EMFILE;
                    }
                    *slot = Some(file);
                    return FD_BASE as i64 + i as i64;
                }
            }
            EMFILE
        }
        Err(e) => e.to_errno() as i64,
    }
}

/// `sys_lseek()`: reposition an open file descriptor.
fn sys_lseek(fd: u64, offset: i64, whence: u32) -> i64 {
    if fd < FD_BASE {
        return EBADF;
    }
    let idx = (fd - FD_BASE) as usize;
    let mut tbl = FD_TABLE.lock();
    let file = match tbl.get_mut(idx).and_then(|o| o.as_mut()) {
        Some(f) => f,
        None => return EBADF,
    };
    match crate::fs::vfs::vfs_llseek(file, offset, whence) {
        Ok(pos) => pos as i64,
        Err(e) => e.to_errno() as i64,
    }
}

/// Writes `bytes` at user address `addr`.
///
/// # Safety
///
/// `addr..addr+bytes.len()` must be a writable user buffer.
unsafe fn write_user_le(addr: u64, bytes: &[u8]) {
    // SAFETY: the caller guarantees the buffer is writable.
    unsafe {
        for (i, &b) in bytes.iter().enumerate() {
            (addr as *mut u8).add(i).write_volatile(b);
        }
    }
}

/// Writes one byte at user address `addr`.
///
/// # Safety
///
/// `addr` must be a writable user address.
unsafe fn write_user_byte(addr: u64, b: u8) {
    // SAFETY: the caller guarantees the address is writable.
    unsafe { (addr as *mut u8).write_volatile(b) };
}

/// `sys_newstat()`: fill the user's `struct stat` for `path`. Only the fields
/// rinux tracks (mode, size, nlink, a nominal block size) are set; the x86-64
/// layout places `st_mode` at offset 24 and `st_size` at 48.
fn sys_stat(path_ptr: u64, statbuf: u64) -> i64 {
    // SAFETY: syscall context; the path is a user pointer read until NUL.
    let path = unsafe { copy_user_cstr(path_ptr, 256) };
    let (mode, size) = match crate::fs::vfs::vfs_stat(&path) {
        Ok(t) => t,
        Err(e) => return e.to_errno() as i64,
    };
    // SAFETY: syscall context on the user's page tables; a bad buffer faults.
    unsafe {
        for i in 0..144u64 {
            (statbuf as *mut u8).add(i as usize).write_volatile(0);
        }
        write_user_le(statbuf + 16, &1u64.to_le_bytes()); // st_nlink
        write_user_le(statbuf + 24, &mode.to_le_bytes()); // st_mode (u32)
        write_user_le(statbuf + 48, &size.to_le_bytes()); // st_size
        write_user_le(statbuf + 56, &4096u64.to_le_bytes()); // st_blksize
    }
    0
}

/// `sys_clock_gettime()`: fill the user's `struct timespec` for a clock.
///
/// A port of `posix_clock_realtime_get()` / the monotonic path of
/// `kernel/time/posix-timers.c`. `CLOCK_REALTIME` reads the wall clock anchored
/// at boot; `CLOCK_MONOTONIC` is time since boot. The x86-64 `struct timespec`
/// is `tv_sec`:i64 at 0 and `tv_nsec`:i64 at 8.
fn sys_clock_gettime(which: u64, tp: u64) -> i64 {
    use crate::kernel::time::jiffies::{HZ, INITIAL_JIFFIES, jiffies};
    use crate::kernel::time::{NSEC_PER_SEC, timekeeping::ktime_get_real_ts64};

    let (sec, nsec) = match which {
        CLOCK_REALTIME => {
            let t = ktime_get_real_ts64();
            (t.tv_sec, t.tv_nsec)
        }
        CLOCK_MONOTONIC => {
            let elapsed = jiffies().wrapping_sub(INITIAL_JIFFIES);
            (
                (elapsed / HZ) as i64,
                (elapsed % HZ) as i64 * (NSEC_PER_SEC / HZ) as i64,
            )
        }
        _ => return EINVAL,
    };
    // SAFETY: syscall context on the user's page tables; a bad buffer faults.
    unsafe {
        write_user_le(tp, &sec.to_le_bytes());
        write_user_le(tp + 8, &nsec.to_le_bytes());
    }
    0
}

/// `sys_gettimeofday()`: fill the user's `struct timeval` with the wall-clock
/// time. A port of the `gettimeofday` path of `kernel/time/time.c`
/// (`do_gettimeofday`/`ktime_get_real_ts64`). The x86-64 `struct timeval` is
/// `tv_sec`:i64 at 0 and `tv_usec`:i64 at 8; the optional `timezone` argument
/// is obsolete and ignored.
fn sys_gettimeofday(tv: u64, _tz: u64) -> i64 {
    use crate::kernel::time::timekeeping::ktime_get_real_ts64;

    if tv == 0 {
        return 0;
    }
    let t = ktime_get_real_ts64();
    let usec = t.tv_nsec / 1000;
    // SAFETY: syscall context on the user's page tables; a bad buffer faults.
    unsafe {
        write_user_le(tv, &t.tv_sec.to_le_bytes());
        write_user_le(tv + 8, &usec.to_le_bytes());
    }
    0
}

/// `sys_nanosleep()`: suspend the caller for at least the requested time.
///
/// A simplified port of `kernel/time/hrtimer.c` (`sys_nanosleep` ->
/// `hrtimer_nanosleep` -> `do_nanosleep`): rinux has one user task and a
/// jiffies tick, so rather than arming an `hrtimer` and scheduling out, it
/// rounds the request up to whole jiffies and waits for the tick to reach the
/// target (interrupts stay on, so `jiffies` advances). The optional remaining
/// time is reported as zero, since the wait always completes in full here.
fn sys_nanosleep(rqtp: u64, rmtp: u64) -> i64 {
    use crate::kernel::time::NSEC_PER_SEC;
    use crate::kernel::time::jiffies::{HZ, jiffies};

    if rqtp == 0 {
        return EINVAL;
    }
    // SAFETY: syscall context; `rqtp` is a user `struct timespec` (tv_sec:i64
    // at 0, tv_nsec:i64 at 8), read directly, and a bad pointer faults.
    let ts = unsafe { copy_from_user(rqtp, 16) };
    let sec = i64::from_le_bytes(ts[0..8].try_into().unwrap());
    let nsec = i64::from_le_bytes(ts[8..16].try_into().unwrap());
    if sec < 0 || !(0..NSEC_PER_SEC as i64).contains(&nsec) {
        return EINVAL;
    }

    // Round the request up to whole jiffies (`timespec64_to_jiffies`). jiffies
    // is a monotonic 64-bit counter, so a plain comparison suffices.
    let ns_per_tick = (NSEC_PER_SEC / HZ) as i64;
    let ticks = sec as u64 * HZ + (nsec + ns_per_tick - 1) as u64 / ns_per_tick as u64;
    let deadline = jiffies() + ticks;
    // The SYSCALL entry masks interrupts, which would freeze `jiffies`. Enable
    // them so the timer tick advances, waiting with `hlt` between ticks, then
    // restore the interrupts-off state the entry runs in.
    if jiffies() < deadline {
        use crate::arch::x86::include::irqflags::{local_irq_disable, native_safe_halt};
        while jiffies() < deadline {
            native_safe_halt();
        }
        local_irq_disable();
    }

    if rmtp != 0 {
        // SAFETY: syscall context; the remaining-time buffer is user memory.
        unsafe {
            write_user_le(rmtp, &0i64.to_le_bytes());
            write_user_le(rmtp + 8, &0i64.to_le_bytes());
        }
    }
    0
}

/// `sys_getrandom()`: fill the user buffer with random bytes.
///
/// A port of the `getrandom(2)` handler of `drivers/char/random.c`. rinux's
/// pool never blocks, so the blocking flags are accepted but have no effect;
/// an unknown flag is rejected. Returns the number of bytes written.
fn sys_getrandom(buf: u64, count: usize, flags: u64) -> i64 {
    if flags & !(GRND_NONBLOCK | GRND_RANDOM | GRND_INSECURE) != 0 {
        return EINVAL;
    }
    if count == 0 {
        return 0;
    }
    // Cap a single request so a bogus count cannot exhaust the allocator.
    let n = count.min(256);
    let mut tmp = alloc::vec![0u8; n];
    crate::drivers::char::random::get_random_bytes(&mut tmp);
    // SAFETY: syscall context on the user's page tables; a bad buffer faults.
    unsafe {
        write_user_le(buf, &tmp);
    }
    n as i64
}

/// `sys_sysinfo()`: fill the user's `struct sysinfo` with system statistics.
///
/// A port of `do_sysinfo()` (`kernel/sys.c`) with `si_meminfo()`
/// (`mm/show_mem.c`): report the uptime, total and free RAM (in pages, with
/// `mem_unit` = page size), and a process count. The x86-64 `struct sysinfo`
/// places `uptime` at 0, `loads[3]` at 8, `totalram` at 32, `freeram` at 40,
/// `procs` (u16) at 80 and `mem_unit` (u32) at 104, for a total of 112 bytes.
///
/// Simplified: load averages are zero (rinux tracks none), swap and the
/// shared/buffer/high counters are zero, and `do_sysinfo`'s unit-scaling loop
/// (only relevant on 32-bit with huge memory) is omitted.
fn sys_sysinfo(info: u64) -> i64 {
    use crate::arch::x86::include::page::PAGE_SIZE;
    use crate::kernel::time::jiffies::{HZ, INITIAL_JIFFIES, jiffies};
    use crate::mm::page_alloc::{nr_free_pages, totalram_pages};

    let uptime = (jiffies().wrapping_sub(INITIAL_JIFFIES) / HZ) as i64;
    // SAFETY: syscall context on the user's page tables; a bad buffer faults.
    unsafe {
        for i in 0..112u64 {
            (info as *mut u8).add(i as usize).write_volatile(0);
        }
        write_user_le(info, &uptime.to_le_bytes()); // uptime (seconds)
        write_user_le(info + 32, &totalram_pages().to_le_bytes()); // totalram
        write_user_le(info + 40, &nr_free_pages().to_le_bytes()); // freeram
        write_user_le(info + 80, &2u16.to_le_bytes()); // procs (swapper + init)
        write_user_le(info + 104, &(PAGE_SIZE as u32).to_le_bytes()); // mem_unit
    }
    0
}

/// `sys_capget()`: report a task's capability sets.
///
/// A port of `kernel/capability.c`'s `capget` (with `cap_validate_magic`):
/// validate the header's ABI version (rewriting it to v3 and failing with
/// `-EINVAL` when unknown, so the caller can retry), then write the target's
/// effective/permitted/inheritable sets as the legacy pairs of 32-bit halves.
///
/// Simplified: rinux runs a single privileged task, so every valid pid is
/// treated as that task -- the full capability set is permitted and effective,
/// with nothing inheritable (as a root init has).
fn sys_capget(header: u64, dataptr: u64) -> i64 {
    if header == 0 {
        return EFAULT;
    }
    // SAFETY: syscall context on the user's page tables; a bad pointer faults.
    let hdr = unsafe { copy_from_user(header, 8) };
    let version = u32::from_le_bytes(hdr[0..4].try_into().unwrap());
    let tocopy = match version {
        LINUX_CAPABILITY_VERSION_1 => 1usize,
        LINUX_CAPABILITY_VERSION_2 | LINUX_CAPABILITY_VERSION_3 => 2usize,
        _ => {
            // Tell the caller which version to use, then fail (or succeed if
            // this was just a version probe with a NULL data pointer).
            // SAFETY: the header is a writable user pointer.
            unsafe { write_user_le(header, &LINUX_CAPABILITY_VERSION_3.to_le_bytes()) };
            return if dataptr == 0 { 0 } else { EINVAL };
        }
    };
    if dataptr == 0 {
        return 0;
    }
    let pid = i32::from_le_bytes(hdr[4..8].try_into().unwrap());
    if pid < 0 {
        return EINVAL;
    }

    let cred = crate::kernel::cred::current_cred();
    let (pe, pp, pi) = (cred.cap_effective, cred.cap_permitted, cred.cap_inheritable);
    // SAFETY: syscall context; `dataptr` is a writable user array of `tocopy`
    // 12-byte `__user_cap_data_struct` records {effective, permitted,
    // inheritable}, each a 32-bit half of the 64-bit set.
    unsafe {
        for i in 0..tocopy {
            let off = dataptr + (i * 12) as u64;
            let shift = 32 * i as u32;
            write_user_le(off, &((pe >> shift) as u32).to_le_bytes());
            write_user_le(off + 4, &((pp >> shift) as u32).to_le_bytes());
            write_user_le(off + 8, &((pi >> shift) as u32).to_le_bytes());
        }
    }
    0
}

/// `sys_getcwd()`: write the current working directory into the user buffer.
///
/// A port of `fs/d_path.c`'s `getcwd`: the path, including its NUL terminator,
/// is copied to `buf` and its length returned, or `-ERANGE` when it does not
/// fit. rinux's single task sits at the root, so the cwd is taken straight
/// from the task's [`fs_struct`](crate::fs::fs_struct) rather than walked back
/// up the dentry tree.
fn sys_getcwd(buf: u64, size: u64) -> i64 {
    let pwd = crate::fs::fs_struct::current_fs().pwd;
    let len = pwd.len() + 1; // including the NUL terminator
    if len as u64 > size {
        return ERANGE;
    }
    // SAFETY: syscall context on the user's page tables; a bad buffer faults.
    unsafe {
        write_user_le(buf, pwd);
        write_user_byte(buf + pwd.len() as u64, 0);
    }
    len as i64
}

/// `sys_mmap()`: map anonymous memory (`ksys_mmap_pgoff` -> `do_mmap`). On
/// x86-64 the offset is in bytes and must be page aligned; file-backed
/// mappings are not supported, so `fd` is ignored and such requests fail.
fn sys_mmap(addr: u64, len: u64, prot: u64, flags: u64, _fd: u64, off: u64) -> i64 {
    use crate::arch::x86::include::page::PAGE_SIZE;
    if off & (PAGE_SIZE - 1) != 0 {
        return EINVAL;
    }
    crate::mm::mmap::do_mmap(addr, len, prot, flags)
}

/// `sys_futex()`: the futex multiplexer (`do_futex()` in
/// `kernel/futex/syscalls.c`), dispatching `FUTEX_WAIT` (with an optional
/// relative `struct timespec` timeout) and `FUTEX_WAKE`; the private and
/// clock flags are accepted and ignored, other operations are `-ENOSYS`.
fn sys_futex(uaddr: u64, op: u64, val: u64, utime: u64, _uaddr2: u64, _val3: u64) -> i64 {
    use crate::kernel::futex::{FUTEX_CMD_MASK, FUTEX_WAIT, FUTEX_WAKE, futex_wait, futex_wake};
    use crate::kernel::time::NSEC_PER_SEC;
    use crate::kernel::time::jiffies::HZ;

    // A futex word is a naturally aligned u32.
    if uaddr & 3 != 0 {
        return EINVAL;
    }
    match (op as u32) & FUTEX_CMD_MASK {
        FUTEX_WAIT => {
            let timeout = if utime != 0 {
                // SAFETY: syscall context; `utime` is a user `struct timespec`
                // (tv_sec:i64 at 0, tv_nsec:i64 at 8).
                let ts = unsafe { copy_from_user(utime, 16) };
                let sec = i64::from_le_bytes(ts[0..8].try_into().unwrap());
                let nsec = i64::from_le_bytes(ts[8..16].try_into().unwrap());
                if sec < 0 || !(0..NSEC_PER_SEC as i64).contains(&nsec) {
                    return EINVAL;
                }
                // Round the relative timeout up to whole jiffies.
                let per_tick = (NSEC_PER_SEC / HZ) as i64;
                Some(sec as u64 * HZ + ((nsec + per_tick - 1) / per_tick) as u64)
            } else {
                None
            };
            futex_wait(uaddr, val as u32, timeout)
        }
        FUTEX_WAKE => futex_wake(uaddr, val as u32),
        _ => ENOSYS,
    }
}

/// `sys_getppid()`: the parent's pid. init was started by the idle task, so
/// its parent is pid 0.
fn sys_getppid() -> i64 {
    0
}

/// `sys_set_tid_address()`: record where to clear the thread id on exit
/// (`task->clear_child_tid`, `kernel/fork.c`) and return the thread id.
fn sys_set_tid_address(tidptr: u64) -> i64 {
    CLEAR_CHILD_TID.store(tidptr, core::sync::atomic::Ordering::Relaxed);
    // SAFETY: current() is the task that made the syscall.
    unsafe { (*current()).pid as i64 }
}

/// `sys_set_robust_list()`: register the thread's robust-futex list head
/// (`kernel/futex/syscalls.c`). `len` must be `sizeof(struct
/// robust_list_head)`: three pointers, 24 bytes.
fn sys_set_robust_list(head: u64, len: u64) -> i64 {
    if len != 24 {
        return EINVAL;
    }
    ROBUST_LIST_HEAD.store(head, core::sync::atomic::Ordering::Relaxed);
    0
}

/// `sys_prlimit64()`: get and/or set one of a process's resource limits.
///
/// A port of `kernel/sys.c`'s `prlimit64` with `do_prlimit`: `old_rlim`, if
/// given, receives the current `struct rlimit64` (`rlim_cur`\@0,
/// `rlim_max`\@8); `new_rlim`, if given, replaces it, rejecting a soft limit
/// above the hard one. rinux has one (root) task, so `pid` 0 and its own pid
/// are accepted and the capability check for raising a hard limit is skipped.
fn sys_prlimit64(pid: i64, resource: u64, new_rlim: u64, old_rlim: u64) -> i64 {
    use crate::kernel::resource::{RLIM_NLIMITS, RLIMITS, Rlimit};

    if resource >= RLIM_NLIMITS as u64 {
        return EINVAL;
    }
    // SAFETY: current() is the calling task.
    let me = unsafe { (*current()).pid as i64 };
    if pid != 0 && pid != me {
        return ESRCH;
    }
    let mut table = RLIMITS.lock();
    let old = table[resource as usize];
    if new_rlim != 0 {
        // SAFETY: syscall context; `new_rlim` is a 16-byte user `rlimit64`.
        let b = unsafe { copy_from_user(new_rlim, 16) };
        let new = Rlimit {
            rlim_cur: u64::from_le_bytes(b[0..8].try_into().unwrap()),
            rlim_max: u64::from_le_bytes(b[8..16].try_into().unwrap()),
        };
        if new.rlim_cur > new.rlim_max {
            return EINVAL;
        }
        table[resource as usize] = new;
    }
    if old_rlim != 0 {
        // SAFETY: syscall context; `old_rlim` is a writable 16-byte user buffer.
        unsafe {
            write_user_le(old_rlim, &old.rlim_cur.to_le_bytes());
            write_user_le(old_rlim + 8, &old.rlim_max.to_le_bytes());
        }
    }
    0
}

/// `sys_arch_prctl()`: get or set the FS/GS segment base, which user space
/// uses as its thread pointer (thread-local storage).
///
/// A port of `arch/x86/kernel/process_64.c`'s `do_arch_prctl_64`: write or
/// read the `MSR_FS_BASE`/`MSR_GS_BASE` MSRs. rinux runs a single user task,
/// so the base is held only in the MSR -- it is not saved and restored per
/// task across a context switch.
fn sys_arch_prctl(code: u64, addr: u64) -> i64 {
    use crate::arch::x86::include::msr_index::{MSR_FS_BASE, MSR_GS_BASE};
    use crate::arch::x86::include::processor::{rdmsr, wrmsr};

    const ARCH_SET_GS: u64 = 0x1001;
    const ARCH_SET_FS: u64 = 0x1002;
    const ARCH_GET_FS: u64 = 0x1003;
    const ARCH_GET_GS: u64 = 0x1004;

    // SAFETY: these are the architectural FS/GS base MSRs; the user base is
    // unprivileged state. For GET, `addr` is a writable user pointer.
    unsafe {
        match code {
            ARCH_SET_FS => {
                wrmsr(MSR_FS_BASE as u32, addr);
                0
            }
            ARCH_SET_GS => {
                wrmsr(MSR_GS_BASE as u32, addr);
                0
            }
            ARCH_GET_FS => {
                write_user_le(addr, &rdmsr(MSR_FS_BASE as u32).to_le_bytes());
                0
            }
            ARCH_GET_GS => {
                write_user_le(addr, &rdmsr(MSR_GS_BASE as u32).to_le_bytes());
                0
            }
            _ => EINVAL,
        }
    }
}

/// `sys_rt_sigaction()`: examine and change a signal's handler.
///
/// A port of `kernel/signal.c`'s `rt_sigaction` with `do_sigaction`: the old
/// action is returned through `oldact` and a new one installed from `act`. The
/// x86-64 user `struct sigaction` is `sa_handler`\@0, `sa_flags`\@8,
/// `sa_restorer`\@16 and the 8-byte `sa_mask`\@24. Only the configuration is
/// recorded; rinux does not yet deliver signals.
fn sys_rt_sigaction(sig: u64, act: u64, oldact: u64, sigsetsize: u64) -> i64 {
    use crate::kernel::signal::{KSigaction, NSIG, SIGHAND, SIGKILL, SIGSTOP};

    if sigsetsize != 8 || sig < 1 || sig > NSIG as u64 {
        return EINVAL;
    }
    // SIGKILL and SIGSTOP cannot have their action changed.
    if act != 0 && (sig == SIGKILL as u64 || sig == SIGSTOP as u64) {
        return EINVAL;
    }
    let idx = (sig - 1) as usize;
    let mut table = SIGHAND.lock();
    let old = table[idx];
    if act != 0 {
        // SAFETY: syscall context; `act` is a 32-byte user `struct sigaction`.
        let a = unsafe { copy_from_user(act, 32) };
        table[idx] = KSigaction {
            handler: u64::from_le_bytes(a[0..8].try_into().unwrap()),
            flags: u64::from_le_bytes(a[8..16].try_into().unwrap()),
            restorer: u64::from_le_bytes(a[16..24].try_into().unwrap()),
            mask: u64::from_le_bytes(a[24..32].try_into().unwrap()),
        };
    }
    if oldact != 0 {
        // SAFETY: syscall context; `oldact` is a writable user buffer.
        unsafe {
            write_user_le(oldact, &old.handler.to_le_bytes());
            write_user_le(oldact + 8, &old.flags.to_le_bytes());
            write_user_le(oldact + 16, &old.restorer.to_le_bytes());
            write_user_le(oldact + 24, &old.mask.to_le_bytes());
        }
    }
    0
}

/// `sys_rt_sigprocmask()`: examine and change the blocked-signal mask.
///
/// A port of `kernel/signal.c`'s `rt_sigprocmask`: `oldset` receives the
/// current mask, and `set` is applied per `how` (`SIG_BLOCK`/`SIG_UNBLOCK`/
/// `SIG_SETMASK`), with `SIGKILL`/`SIGSTOP` never left blocked.
fn sys_rt_sigprocmask(how: u64, set: u64, oldset: u64, sigsetsize: u64) -> i64 {
    use crate::kernel::signal::{BLOCKED, SIG_BLOCK, SIG_SETMASK, SIG_UNBLOCK, SIG_UNBLOCKABLE};
    use core::sync::atomic::Ordering;

    if sigsetsize != 8 {
        return EINVAL;
    }
    let cur = BLOCKED.load(Ordering::Relaxed);
    if oldset != 0 {
        // SAFETY: syscall context; `oldset` is a writable 8-byte user buffer.
        unsafe { write_user_le(oldset, &cur.to_le_bytes()) };
    }
    if set != 0 {
        // SAFETY: syscall context; `set` is an 8-byte user sigset.
        let bytes = unsafe { copy_from_user(set, 8) };
        let newset = u64::from_le_bytes(bytes.try_into().unwrap());
        let next = match how {
            SIG_BLOCK => cur | newset,
            SIG_UNBLOCK => cur & !newset,
            SIG_SETMASK => newset,
            _ => return EINVAL,
        };
        BLOCKED.store(next & !SIG_UNBLOCKABLE, Ordering::Relaxed);
    }
    0
}

/// `sys_kill()`: raise signal `sig` on a process. rinux runs a single task,
/// so the target pid is not checked -- any positive signal is made pending on
/// it. A port of the `kill`/`do_send_sig_info` path of `kernel/signal.c`.
fn sys_kill(_pid: i64, sig: u64) -> i64 {
    use crate::kernel::signal::{NSIG, send_signal};
    if sig == 0 {
        return 0; // signal 0 only probes for existence
    }
    if sig > NSIG as u64 {
        return EINVAL;
    }
    send_signal(sig as u32);
    0
}

/// `sys_getdents64()`: read directory entries from an open directory fd into
/// the user buffer as packed `struct linux_dirent64` records
/// (`d_ino`:u64, `d_off`:u64, `d_reclen`:u16, `d_type`:u8, `d_name`:cstr).
/// Returns the bytes written, or 0 at the end of the directory. The file's
/// position is used as the index of the next entry to return.
fn sys_getdents64(fd: u64, buf: u64, count: usize) -> i64 {
    if fd < FD_BASE {
        return EBADF;
    }
    let idx_start;
    let dir_ino;
    {
        let mut tbl = FD_TABLE.lock();
        let file = match tbl
            .get_mut((fd - FD_BASE) as usize)
            .and_then(|o| o.as_mut())
        {
            Some(f) => f,
            None => return EBADF,
        };
        idx_start = file.pos;
        dir_ino = file.ino;
    }
    let entries = match crate::fs::vfs::vfs_readdir_ino(dir_ino) {
        Ok(e) => e,
        Err(e) => return e.to_errno() as i64,
    };

    let mut written = 0usize;
    let mut idx = idx_start as usize;
    while idx < entries.len() {
        let e = &entries[idx];
        // Fixed part is 19 bytes; add the name, a NUL, and pad to 8.
        let reclen = (19 + e.name.len() + 1 + 7) & !7;
        if written + reclen > count {
            break;
        }
        let rec = buf + written as u64;
        // SAFETY: the record fits within the user buffer bound just checked.
        unsafe {
            write_user_le(rec, &e.ino.to_le_bytes()); // d_ino
            write_user_le(rec + 8, &((idx as u64 + 1).to_le_bytes())); // d_off
            write_user_le(rec + 16, &(reclen as u16).to_le_bytes()); // d_reclen
            write_user_byte(rec + 18, e.d_type); // d_type
            for (i, &b) in e.name.iter().enumerate() {
                write_user_byte(rec + 19 + i as u64, b);
            }
            write_user_byte(rec + 19 + e.name.len() as u64, 0); // NUL
        }
        written += reclen;
        idx += 1;
    }
    // Advance the position to the next unread entry.
    if let Some(file) = FD_TABLE
        .lock()
        .get_mut((fd - FD_BASE) as usize)
        .and_then(|o| o.as_mut())
    {
        file.pos = idx as u64;
    }
    written as i64
}

/// `sys_close()`: drop a descriptor (the console descriptors are no-ops).
fn sys_close(fd: u64) -> i64 {
    if fd < FD_BASE {
        return 0;
    }
    let idx = (fd - FD_BASE) as usize;
    if let Some(slot) = FD_TABLE.lock().get_mut(idx) {
        *slot = None;
    }
    0
}

/// Copies a NUL-terminated string from user address `uaddr` (at most `max`
/// bytes) into a kernel `Vec`.
///
/// # Safety
///
/// Runs on the user task's page tables; a bad address faults.
unsafe fn copy_user_cstr(uaddr: u64, max: usize) -> alloc::vec::Vec<u8> {
    let mut v = alloc::vec::Vec::new();
    for i in 0..max {
        // SAFETY: see the function contract.
        let b = unsafe { (uaddr as *const u8).add(i).read_volatile() };
        if b == 0 {
            break;
        }
        v.push(b);
    }
    v
}

/// `sys_newuname()`: fill the user's `struct new_utsname` (six 65-byte
/// NUL-terminated fields: sysname, nodename, release, version, machine,
/// domainname).
fn sys_uname(buf: u64) -> i64 {
    const UTS_FIELD_LEN: usize = 65;
    let fields: [&[u8]; 6] = [
        b"Linux",
        b"rinux",
        crate::init::version::UTS_RELEASE.as_bytes(),
        b"#1 rinux",
        b"x86_64",
        b"(none)",
    ];
    for (i, f) in fields.iter().enumerate() {
        let base = buf + (i * UTS_FIELD_LEN) as u64;
        let n = f.len().min(UTS_FIELD_LEN - 1);
        // SAFETY: syscall context on the user's page tables; a bad buffer
        // faults and the task is killed. Each field is NUL-padded.
        unsafe {
            for j in 0..UTS_FIELD_LEN {
                (base as *mut u8).add(j).write_volatile(0);
            }
            for (j, &b) in f.iter().take(n).enumerate() {
                (base as *mut u8).add(j).write_volatile(b);
            }
        }
    }
    0
}

/// `sys_getpid()`.
fn sys_getpid() -> i64 {
    // SAFETY: current is the running task.
    unsafe { (*current()).pid as i64 }
}

/// `sys_brk()`: grow the user heap. The initial break is set by the ELF
/// loader; this maps pages up to `addr` and returns the new break.
fn sys_brk(addr: u64) -> i64 {
    crate::fs::exec::do_brk(addr) as i64
}

/// `sys_exit()`/`sys_exit_group()`: end the current user task.
fn sys_exit(code: i32) -> i64 {
    USER_EXIT_CODE.store(code as i64, core::sync::atomic::Ordering::Release);
    // SAFETY: current is the running user task.
    let pid = unsafe { (*current()).pid };
    pr_info!("init: user process (pid {}) exited with code {}", pid, code);
    // futex_exit_release(): release any robust futexes the dying task held.
    let robust = ROBUST_LIST_HEAD.load(core::sync::atomic::Ordering::Relaxed);
    if robust != 0 {
        let released = crate::kernel::futex::exit_robust_list(robust, pid as u32);
        pr_info!(
            "exit: robust futex list walked, {} lock(s) released",
            released
        );
    }
    // mm_release(): zero the thread id the task registered with
    // set_tid_address(), so a joiner sees it has gone, and wake anyone waiting
    // on it (pthread_join).
    let tidptr = CLEAR_CHILD_TID.load(core::sync::atomic::Ordering::Relaxed);
    if tidptr != 0 {
        // SAFETY: syscall context on the user's page tables; a bad pointer
        // faults, as it would in put_user().
        let was = unsafe { u32::from_le_bytes(copy_from_user(tidptr, 4).try_into().unwrap()) };
        unsafe { write_user_le(tidptr, &0u32.to_le_bytes()) };
        let woken = crate::kernel::futex::futex_wake(tidptr, 1);
        pr_info!(
            "exit: cleared child tid at {:#x} (was {}), woke {}",
            tidptr,
            was,
            woken
        );
    }
    crate::kernel::sched::core::do_exit(code);
}

// SPDX-License-Identifier: GPL-2.0
//! A small procfs.
//!
//! A partial port of `fs/proc/`. procfs is a virtual filesystem whose files
//! have no stored data: each is generated on read by a function, which is
//! exactly what `proc_create_single()` with a `show` callback does. rinux
//! grafts `/proc` into the rootfs tree (Linux mounts it) and registers a
//! few files; the seq_file machinery is reduced to "return a `Vec<u8>`".
//!
//! Not ported: per-process `/proc/<pid>`, the full `proc_ops`/`seq_operations`,
//! sysctl and the mount itself.

use alloc::format;
use alloc::vec::Vec;

use rinux_lib::errno::Result;

use crate::fs::vfs::{InodeKind, graft, proc_read_at};
use crate::include::linux::fs::{FileOperations, S_IFDIR, S_IFREG};

/// `proc_file_operations`: read dispatches to the inode's generator.
pub struct ProcFop;

impl FileOperations for ProcFop {
    fn read(&self, ino: u64, pos: u64, buf: &mut [u8]) -> Result<usize> {
        proc_read_at(ino, pos, buf)
    }
}

/// The shared procfs file-operations instance.
pub static PROC_FOP: ProcFop = ProcFop;

/// `/proc/version`.
fn version() -> Vec<u8> {
    let release = crate::init::version::UTS_RELEASE;
    format!("Linux version {release} (rinux@rinux) (rustc) #1\n").into_bytes()
}

/// `/proc/meminfo`.
fn meminfo() -> Vec<u8> {
    use crate::arch::x86::include::page::PAGE_SIZE;
    use crate::mm::page_alloc::{nr_free_pages, totalram_pages};
    let total = totalram_pages() * PAGE_SIZE / 1024;
    let free = nr_free_pages() * PAGE_SIZE / 1024;
    format!("MemTotal:       {total:>8} kB\nMemFree:        {free:>8} kB\n").into_bytes()
}

/// `/proc/uptime`.
fn uptime() -> Vec<u8> {
    use crate::kernel::time::jiffies::{HZ, jiffies};
    let ticks = jiffies().wrapping_sub(crate::kernel::time::jiffies::INITIAL_JIFFIES);
    let secs = ticks / HZ;
    let hundredths = (ticks % HZ) * 100 / HZ;
    format!("{secs}.{hundredths:02} 0.00\n").into_bytes()
}

/// `/proc/cmdline`.
fn cmdline() -> Vec<u8> {
    let cl = crate::init::main::boot_command_line();
    let mut v = cl[..rinux_lib::string::strlen(&cl)].to_vec();
    v.push(b'\n');
    v
}

/// `/proc/cpuinfo`: `show_cpuinfo()` from `arch/x86/kernel/cpu/proc.c`, for
/// the one boot CPU. The feature `flags` line is omitted.
fn cpuinfo() -> Vec<u8> {
    use crate::arch::x86::kernel::cpu::common::boot_cpu_data;
    use rinux_lib::string::strlen;
    let c = boot_cpu_data();
    let vendor = &c.x86_vendor_id[..strlen(&c.x86_vendor_id)];
    let model_id = &c.x86_model_id[..strlen(&c.x86_model_id)];

    let mut v = Vec::new();
    v.extend_from_slice(b"processor\t: 0\n");
    v.extend_from_slice(b"vendor_id\t: ");
    v.extend_from_slice(vendor);
    v.extend_from_slice(
        format!("\ncpu family\t: {}\nmodel\t\t: {}\n", c.x86, c.x86_model).as_bytes(),
    );
    v.extend_from_slice(b"model name\t: ");
    v.extend_from_slice(model_id);
    v.extend_from_slice(
        format!(
            "\nstepping\t: {}\ncpuid level\t: {}\n\n",
            c.x86_stepping, c.cpuid_level
        )
        .as_bytes(),
    );
    v
}

/// `/proc/stat`: a reduced `show_stat()` from `fs/proc/stat.c` -- the idle
/// time is the whole uptime (rinux has no per-mode accounting), plus the
/// context-switch count and the number of running tasks.
fn stat() -> Vec<u8> {
    use crate::kernel::sched::core::{for_each_running_pid, nr_context_switches};
    use crate::kernel::time::jiffies::{INITIAL_JIFFIES, jiffies};
    let ticks = jiffies().wrapping_sub(INITIAL_JIFFIES);
    let ctxt = nr_context_switches();
    let mut nproc = 0u64;
    for_each_running_pid(|_, _| nproc += 1);
    format!(
        "cpu  0 0 0 {ticks} 0 0 0 0 0 0\ncpu0 0 0 0 {ticks} 0 0 0 0 0 0\nctxt {ctxt}\nbtime 0\nprocesses {nproc}\n"
    )
    .into_bytes()
}

/// `/proc/loadavg`: `loadavg_proc_show()` from `fs/proc/loadavg.c`. rinux
/// does not compute load averages, so they read as zero.
fn loadavg() -> Vec<u8> {
    use crate::kernel::sched::core::for_each_running_pid;
    let mut running = 0u32;
    for_each_running_pid(|_, _| running += 1);
    format!("0.00 0.00 0.00 {running}/{running} 0\n").into_bytes()
}

/// `/proc/filesystems`: the registered filesystem types (`filesystems.c`).
fn filesystems() -> Vec<u8> {
    b"nodev\tproc\nnodev\tramfs\n\text2\n\text4\n".to_vec()
}

/// `proc_root_init()`: create `/proc` and register its files.
pub fn proc_root_init() {
    let _ = crate::fs::vfs::vfs_mkdir(b"/proc", 0o555);
    let _ = graft(
        b"/proc",
        b"version",
        S_IFREG | 0o444,
        &PROC_FOP,
        InodeKind::Proc(version),
    );
    let _ = graft(
        b"/proc",
        b"meminfo",
        S_IFREG | 0o444,
        &PROC_FOP,
        InodeKind::Proc(meminfo),
    );
    let _ = graft(
        b"/proc",
        b"uptime",
        S_IFREG | 0o444,
        &PROC_FOP,
        InodeKind::Proc(uptime),
    );
    let _ = graft(
        b"/proc",
        b"cmdline",
        S_IFREG | 0o444,
        &PROC_FOP,
        InodeKind::Proc(cmdline),
    );
    for (name, generate) in [
        (b"cpuinfo".as_slice(), cpuinfo as fn() -> Vec<u8>),
        (b"stat".as_slice(), stat),
        (b"loadavg".as_slice(), loadavg),
        (b"filesystems".as_slice(), filesystems),
    ] {
        let _ = graft(
            b"/proc",
            name,
            S_IFREG | 0o444,
            &PROC_FOP,
            InodeKind::Proc(generate),
        );
    }
    let _ = S_IFDIR;
}

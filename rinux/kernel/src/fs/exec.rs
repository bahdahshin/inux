// SPDX-License-Identifier: GPL-2.0-only
//! `execve` and the program break.
//!
//! A simplified port of `do_execveat_common()`/`bprm_execve()` from
//! `fs/exec.c`: read an executable from the filesystem, load it with the
//! ELF loader, install its address space on the current task, and enter
//! user mode. There is only one binary format (ELF) and no argv/envp from
//! the caller beyond `argv[0]`.
//!
//! Copyright (C) 1991, 1992  Linus Torvalds

use core::sync::atomic::{AtomicU64, Ordering};

use rinux_lib::errno::Result;

use crate::arch::x86::include::page::PAGE_SIZE;
use crate::arch::x86::include::pgtable::{_PAGE_NX, _PAGE_PRESENT, _PAGE_RW, _PAGE_USER};
use crate::arch::x86::kernel::process::start_thread;
use crate::arch::x86::mm::user::{USER_LOAD_BASE, alloc_user_range};
use crate::fs::binfmt_elf::load_elf_binary;
use crate::fs::vfs::read_file;
use crate::include::linux::sched::current;

/// The current user program break, for `sys_brk`.
static CURRENT_BRK: AtomicU64 = AtomicU64::new(0);
/// The user PGD of the current exec, for mapping new brk pages.
static CURRENT_USER_PGD: AtomicU64 = AtomicU64::new(0);

/// `kernel_execve()`: load and run the program at `path`. On success it
/// enters user mode and does not return.
pub fn kernel_execve(path: &[u8]) -> Result {
    let data = read_file(path)?;
    let loaded = load_elf_binary(&data, path)?;

    CURRENT_BRK.store(loaded.brk, Ordering::Relaxed);
    CURRENT_USER_PGD.store(loaded.pgd, Ordering::Relaxed);

    // Install the new address space on the current task.
    // SAFETY: current is the running task; it becomes a user task.
    unsafe {
        (*current()).mm_pgd = loaded.pgd;
        (*current()).flags &= !crate::include::linux::sched::PF_KTHREAD;
    }

    pr_info!(
        "init: executing {} at entry {:#x}",
        crate::kernel::printk::BStr(path),
        loaded.entry
    );
    // start_thread() enters ring 3 and never returns.
    start_thread(loaded.pgd, loaded.entry, loaded.user_sp)
}

/// `sys_brk()` core: grow or query the program break.
pub fn do_brk(addr: u64) -> u64 {
    let cur = CURRENT_BRK.load(Ordering::Relaxed);
    if addr == 0 || addr < cur {
        return cur;
    }
    // Map new pages [cur, addr) into the user address space.
    let pgd = CURRENT_USER_PGD.load(Ordering::Relaxed);
    let flags = _PAGE_PRESENT | _PAGE_RW | _PAGE_USER | _PAGE_NX;
    let start = (cur + PAGE_SIZE - 1) & !(PAGE_SIZE - 1);
    let end = (addr + PAGE_SIZE - 1) & !(PAGE_SIZE - 1);
    if end > start {
        alloc_user_range(pgd, start, end - start, flags);
    }
    CURRENT_BRK.store(addr, Ordering::Relaxed);
    addr
}

const _: () = assert!(USER_LOAD_BASE != 0);

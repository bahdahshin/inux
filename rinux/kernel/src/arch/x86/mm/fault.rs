// SPDX-License-Identifier: GPL-2.0
//! Page faults.
//!
//! A partial port of `arch/x86/mm/fault.c`. There are no user processes,
//! no demand paging and no exception tables yet, so every page fault is a
//! kernel bug and ends in `page_fault_oops()`.
//!
//! Copyright (C) 1995  Linus Torvalds
//! Copyright (C) 2001, 2002 Andi Kleen, SuSE Labs.
//! Copyright (C) 2008-2009, Red Hat Inc., Ingo Molnar

use core::fmt::Write;

use crate::arch::x86::include::processor::{read_cr2, read_cr3};
use crate::arch::x86::include::ptrace::{PtRegs, user_mode};
use crate::arch::x86::kernel::dumpstack::{__die, oops_begin, oops_end};
use crate::arch::x86::kernel::traps::SIGKILL;
use crate::kernel::printk::{FmtBuf, LOGLEVEL_DEFAULT};

// enum x86_pf_error_code, arch/x86/include/asm/trap_pf.h
pub const X86_PF_PROT: u64 = 1 << 0;
pub const X86_PF_WRITE: u64 = 1 << 1;
pub const X86_PF_USER: u64 = 1 << 2;
pub const X86_PF_RSVD: u64 = 1 << 3;
pub const X86_PF_INSTR: u64 = 1 << 4;
pub const X86_PF_PK: u64 = 1 << 5;
pub const X86_PF_SHSTK: u64 = 1 << 6;
pub const X86_PF_SGX: u64 = 1 << 15;
pub const X86_PF_RMP: u64 = 1 << 31;

const PAGE_SIZE: u64 = 4096;
/// `_PAGE_PRESENT`.
const _PAGE_PRESENT: u64 = 1 << 0;
/// `_PAGE_PSE`: a large page at this level.
const _PAGE_PSE: u64 = 1 << 7;
/// `PTE_PFN_MASK` for 52-bit physical addresses.
const PTE_PFN_MASK: u64 = 0x000f_ffff_ffff_f000;
/// The boot page tables identity-map the first 4 GiB.
const IDENTITY_MAPPED: u64 = 1 << 32;

/// Reads entry `index` of the page table at physical address `table`, if
/// that table is in identity-mapped memory (`bad_address()` in C).
fn read_entry(table: u64, index: u64) -> Option<u64> {
    let addr = (table & PTE_PFN_MASK) + index * 8;
    if addr + 8 > IDENTITY_MAPPED {
        return None;
    }
    // SAFETY: addr is an aligned page table entry in identity-mapped RAM.
    Some(unsafe { (addr as *const u64).read_volatile() })
}

/// `dump_pagetable()`: the page table entries that translate `address`.
fn dump_pagetable(address: u64) {
    let mut line = FmtBuf::<160>::new();
    let indices = [
        (address >> 39) & 511,
        (address >> 30) & 511,
        (address >> 21) & 511,
        (address >> 12) & 511,
    ];
    let mut table = read_cr3();

    let mut walk = |line: &mut FmtBuf<160>| -> Option<()> {
        let pgd = read_entry(table, indices[0])?;
        let _ = write!(line, "PGD {pgd:x} ");
        if pgd & _PAGE_PRESENT == 0 {
            return Some(());
        }
        // With 4-level paging the P4D level is folded into the PGD.
        let _ = write!(line, "P4D {pgd:x} ");
        table = pgd;
        let pud = read_entry(table, indices[1])?;
        let _ = write!(line, "PUD {pud:x} ");
        if pud & _PAGE_PRESENT == 0 || pud & _PAGE_PSE != 0 {
            return Some(());
        }
        table = pud;
        let pmd = read_entry(table, indices[2])?;
        let _ = write!(line, "PMD {pmd:x} ");
        if pmd & _PAGE_PRESENT == 0 || pmd & _PAGE_PSE != 0 {
            return Some(());
        }
        table = pmd;
        let pte = read_entry(table, indices[3])?;
        let _ = write!(line, "PTE {pte:x}");
        Some(())
    };

    match walk(&mut line) {
        Some(()) => pr_info!("{}", line.as_str().trim_end()),
        None => pr_info!("BAD"),
    }
}

/// `show_fault_oops()`.
fn show_fault_oops(regs: &PtRegs, error_code: u64, address: u64) {
    if address < PAGE_SIZE && !user_mode(regs) {
        pr_alert!(
            "BUG: kernel NULL pointer dereference, address: {:016x}",
            address
        );
    } else {
        pr_alert!(
            "BUG: unable to handle page fault for address: {:016x}",
            address
        );
    }

    pr_alert!(
        "#PF: {} {} in {} mode",
        if error_code & X86_PF_USER != 0 {
            "user"
        } else {
            "supervisor"
        },
        if error_code & X86_PF_INSTR != 0 {
            "instruction fetch"
        } else if error_code & X86_PF_WRITE != 0 {
            "write access"
        } else {
            "read access"
        },
        if user_mode(regs) { "user" } else { "kernel" }
    );
    pr_alert!(
        "#PF: error_code({:#06x}) - {}",
        error_code,
        if error_code & X86_PF_PROT == 0 {
            "not-present page"
        } else if error_code & X86_PF_RSVD != 0 {
            "reserved bit violation"
        } else if error_code & X86_PF_PK != 0 {
            "protection keys violation"
        } else if error_code & X86_PF_RMP != 0 {
            "RMP violation"
        } else {
            "permissions violation"
        }
    );

    dump_pagetable(address);
}

/// `page_fault_oops()`: the kernel touched a bad page.
fn page_fault_oops(regs: &mut PtRegs, error_code: u64, address: u64) {
    let flags = oops_begin();
    show_fault_oops(regs, error_code, address);

    let mut sig = SIGKILL;
    if __die("Oops", regs, error_code as i64) {
        sig = 0;
    }
    // Executive summary in case the body of the oops scrolled away.
    printk!(LOGLEVEL_DEFAULT, "CR2: {:016x}", address);
    oops_end(flags, regs, sig);
}

/// `exc_page_fault()`.
pub extern "C" fn exc_page_fault(regs: &mut PtRegs, error_code: u64) {
    // CR2 must be read before anything can fault again.
    let address = read_cr2();
    // do_user_addr_fault(): a fault from user mode is a bug in the user
    // program, not the kernel. There is no demand paging, so send the task
    // a fatal signal (we have none yet) by killing it with SIGSEGV, as
    // Linux does after the VMA lookup fails.
    if user_mode(regs) {
        use crate::include::linux::sched::current;
        // SAFETY: current is the faulting user task.
        let pid = unsafe { (*current()).pid };
        pr_err!(
            "init: user process (pid {}) segfault at {:#x} ip {:#x} ({})",
            pid,
            address,
            regs.ip,
            if error_code & X86_PF_WRITE != 0 {
                "write"
            } else {
                "read"
            }
        );
        crate::kernel::syscalls::USER_EXIT_CODE.store(139, core::sync::atomic::Ordering::Release);
        crate::kernel::sched::core::do_exit(139); // 128 + SIGSEGV
    }
    // handle_page_fault() in the kernel: nothing is paged in, so oops.
    page_fault_oops(regs, error_code, address);
}

// SPDX-License-Identifier: GPL-2.0
//! Anonymous memory mappings.
//!
//! A port of the anonymous-mapping core of `mm/mmap.c` (`do_mmap`,
//! `get_unmapped_area`, `do_munmap` with `split_vma`) and `mm/mprotect.c`
//! (`do_mprotect_pkey`): a list of VMAs describes the task's mappings; `mmap`
//! picks an address and maps fresh zeroed pages, `munmap` and `mprotect` split
//! the VMAs at their range's edges and then free or re-flag whole VMAs.
//!
//! Simplified from Linux's: anonymous mappings only (one without
//! `MAP_ANONYMOUS` is refused with `EBADF`, as its fd of -1 would be); a sorted
//! `Vec` in place of the maple tree; top-down placement below the stack gap
//! that never reuses a hole; `MAP_FIXED` only onto unmapped space; pages are
//! populated at `mmap` time rather than on first fault; and one task, so one mm.

use alloc::vec::Vec;
use core::sync::atomic::{AtomicU64, Ordering};

use crate::arch::x86::include::page::PAGE_SIZE;
use crate::arch::x86::include::pgtable::{_PAGE_NX, _PAGE_PRESENT, _PAGE_RW, _PAGE_USER};
use crate::arch::x86::mm::user::{
    USER_STACK_SIZE, USER_STACK_TOP, map_user_page, set_user_page_flags, unmap_user_page,
};
use crate::include::linux::sched::current;
use crate::kernel::locking::SpinLock;
use crate::mm::__GFP_ZERO;
use crate::mm::page_alloc::{alloc_pages, free_pages};

// `include/uapi/asm-generic/mman-common.h`.
pub const PROT_NONE: u64 = 0x0;
pub const PROT_READ: u64 = 0x1;
pub const PROT_WRITE: u64 = 0x2;
pub const PROT_EXEC: u64 = 0x4;
pub const MAP_SHARED: u64 = 0x01;
pub const MAP_PRIVATE: u64 = 0x02;
pub const MAP_FIXED: u64 = 0x10;
pub const MAP_ANONYMOUS: u64 = 0x20;

const EBADF: i64 = -9;
const ENOMEM: i64 = -12;
const EINVAL: i64 = -22;

/// `struct vm_area_struct`: one mapping, `[start, end)` with its protection.
#[derive(Clone, Copy, Debug)]
pub struct Vma {
    pub start: u64,
    pub end: u64,
    pub prot: u64,
}

/// `mm->mm_mt`: the task's mappings, sorted by start address.
static MM: SpinLock<Vec<Vma>> = SpinLock::new(Vec::new());

/// `get_unmapped_area()` state: mappings are handed out top-down starting a
/// 1 MiB gap below the user stack.
static MMAP_TOP: AtomicU64 = AtomicU64::new(USER_STACK_TOP - USER_STACK_SIZE - 0x10_0000);

/// `vm_get_page_prot()`: PTE flags for a `PROT_*` combination. `PROT_NONE`
/// keeps the frame but clears present, so any access faults.
fn prot_to_pte(prot: u64) -> u64 {
    let mut f = _PAGE_USER;
    if prot & (PROT_READ | PROT_WRITE | PROT_EXEC) != 0 {
        f |= _PAGE_PRESENT;
    }
    if prot & PROT_WRITE != 0 {
        f |= _PAGE_RW;
    }
    if prot & PROT_EXEC == 0 {
        f |= _PAGE_NX;
    }
    f
}

fn current_pgd() -> u64 {
    // SAFETY: current() is the user task in the syscall.
    unsafe { (*current()).mm_pgd }
}

fn page_align_up(len: u64) -> Option<u64> {
    len.checked_add(PAGE_SIZE - 1).map(|l| l & !(PAGE_SIZE - 1))
}

/// `split_vma()`: make `addr` a VMA boundary if it falls inside one.
fn split_at(vmas: &mut Vec<Vma>, addr: u64) {
    if let Some(i) = vmas.iter().position(|v| v.start < addr && addr < v.end) {
        let tail = Vma {
            start: addr,
            end: vmas[i].end,
            prot: vmas[i].prot,
        };
        vmas[i].end = addr;
        vmas.insert(i + 1, tail);
    }
}

/// `do_mmap()`: create an anonymous mapping of `len` bytes, returning its
/// address or a negative errno.
pub fn do_mmap(addr: u64, len: u64, prot: u64, flags: u64) -> i64 {
    if len == 0 || prot & !(PROT_READ | PROT_WRITE | PROT_EXEC) != 0 {
        return EINVAL;
    }
    if flags & MAP_ANONYMOUS == 0 {
        return EBADF; // file-backed mappings are not supported
    }
    if flags & (MAP_SHARED | MAP_PRIVATE) == 0 {
        return EINVAL;
    }
    let Some(len) = page_align_up(len) else {
        return ENOMEM;
    };

    let mut vmas = MM.lock();
    let start = if flags & MAP_FIXED != 0 {
        if addr & (PAGE_SIZE - 1) != 0 {
            return EINVAL;
        }
        if vmas.iter().any(|v| v.start < addr + len && addr < v.end) {
            return EINVAL; // replacing an existing mapping is not supported
        }
        addr
    } else {
        // get_unmapped_area(), top-down.
        let end = MMAP_TOP.fetch_sub(len, Ordering::Relaxed);
        end - len
    };

    let pgd = current_pgd();
    let pte = prot_to_pte(prot);
    let mut va = start;
    while va < start + len {
        let page = alloc_pages(__GFP_ZERO, 0);
        if page == 0 {
            // Undo the pages mapped so far.
            let mut back = start;
            while back < va {
                if let Some(phys) = unmap_user_page(pgd, back) {
                    free_pages(phys, 0);
                }
                back += PAGE_SIZE;
            }
            return ENOMEM;
        }
        map_user_page(pgd, va, page, pte);
        va += PAGE_SIZE;
    }

    let pos = vmas
        .iter()
        .position(|v| v.start > start)
        .unwrap_or(vmas.len());
    vmas.insert(
        pos,
        Vma {
            start,
            end: start + len,
            prot,
        },
    );
    start as i64
}

/// `do_munmap()`: remove any mappings in `[addr, addr + len)`, freeing their
/// pages. Unmapped parts of the range are ignored, as in Linux.
pub fn do_munmap(addr: u64, len: u64) -> i64 {
    if addr & (PAGE_SIZE - 1) != 0 || len == 0 {
        return EINVAL;
    }
    let Some(len) = page_align_up(len) else {
        return EINVAL;
    };
    let end = addr + len;
    let pgd = current_pgd();

    let mut vmas = MM.lock();
    split_at(&mut vmas, addr);
    split_at(&mut vmas, end);
    vmas.retain(|v| {
        if v.start >= addr && v.end <= end {
            let mut va = v.start;
            while va < v.end {
                if let Some(phys) = unmap_user_page(pgd, va) {
                    free_pages(phys, 0);
                }
                va += PAGE_SIZE;
            }
            false
        } else {
            true
        }
    });
    0
}

/// `do_mprotect_pkey()`: change the protection of every mapping in
/// `[addr, addr + len)`, which must be fully mapped (else `ENOMEM`).
pub fn do_mprotect(addr: u64, len: u64, prot: u64) -> i64 {
    if addr & (PAGE_SIZE - 1) != 0 || len == 0 || prot & !(PROT_READ | PROT_WRITE | PROT_EXEC) != 0
    {
        return EINVAL;
    }
    let Some(len) = page_align_up(len) else {
        return ENOMEM;
    };
    let end = addr + len;

    let mut vmas = MM.lock();
    // The whole range must be covered by mappings.
    let mut covered = addr;
    for v in vmas.iter() {
        if v.end <= covered {
            continue;
        }
        if v.start > covered {
            break;
        }
        covered = v.end;
        if covered >= end {
            break;
        }
    }
    if covered < end {
        return ENOMEM;
    }

    split_at(&mut vmas, addr);
    split_at(&mut vmas, end);
    let pgd = current_pgd();
    let pte = prot_to_pte(prot);
    for v in vmas.iter_mut().filter(|v| v.start >= addr && v.end <= end) {
        v.prot = prot;
        let mut va = v.start;
        while va < v.end {
            set_user_page_flags(pgd, va, pte);
            va += PAGE_SIZE;
        }
    }
    0
}

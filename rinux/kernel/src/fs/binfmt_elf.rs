// SPDX-License-Identifier: GPL-2.0-only
//! The ELF program loader.
//!
//! A simplified port of `load_elf_binary()` from `fs/binfmt_elf.c`. It reads
//! a static ELF64 executable, maps each `PT_LOAD` segment into a fresh user
//! address space, builds the initial stack (argv/envp/auxv), and returns the
//! entry point and stack pointer for `start_thread`.
//!
//! Not ported: dynamic linking (`PT_INTERP`/ld.so), PIE relocation, core
//! dumps, the `brk`/data segment bookkeeping beyond an initial break, and
//! all the security/randomization machinery. The executable must be a
//! non-PIE `ET_EXEC` linked at [`USER_LOAD_BASE`].

use alloc::vec::Vec;

use rinux_lib::errno::{EINVAL, ENOEXEC, Result};

use crate::arch::x86::include::page::PAGE_SIZE;
use crate::arch::x86::include::pgtable::{_PAGE_NX, _PAGE_PRESENT, _PAGE_RW, _PAGE_USER};
use crate::arch::x86::mm::user::{
    USER_STACK_SIZE, USER_STACK_TOP, alloc_user_range, copy_to_user_pgd, create_user_pgd,
};
use crate::include::uapi::elf::*;

/// The result of loading an ELF: the new address space and where to start.
pub struct LoadedElf {
    pub pgd: u64,
    pub entry: u64,
    pub user_sp: u64,
    /// The program break: the first page-aligned address past the image.
    pub brk: u64,
}

/// `load_elf_binary()`: load `data` (the whole file) into a new user mm.
pub fn load_elf_binary(data: &[u8], argv0: &[u8]) -> Result<LoadedElf> {
    let ehdr: Elf64Ehdr = read_at(data, 0).ok_or(ENOEXEC)?;
    if &ehdr.e_ident[..4] != ELFMAG
        || ehdr.e_ident[4] != ELFCLASS64
        || ehdr.e_ident[5] != ELFDATA2LSB
        || ehdr.e_type != ET_EXEC
        || ehdr.e_machine != EM_X86_64
    {
        return Err(ENOEXEC);
    }

    let pgd = create_user_pgd();
    let mut brk = 0u64;

    // Map each PT_LOAD segment.
    for i in 0..ehdr.e_phnum as usize {
        let off = ehdr.e_phoff as usize + i * ehdr.e_phentsize as usize;
        let ph: Elf64Phdr = read_at(data, off).ok_or(ENOEXEC)?;
        if ph.p_type == PT_INTERP {
            // Dynamic executables are not supported yet.
            return Err(EINVAL);
        }
        if ph.p_type != PT_LOAD || ph.p_memsz == 0 {
            continue;
        }

        let mut flags = _PAGE_PRESENT | _PAGE_USER;
        if ph.p_flags & PF_W != 0 {
            flags |= _PAGE_RW;
        }
        if ph.p_flags & PF_X == 0 {
            flags |= _PAGE_NX;
        }

        // Allocate the whole segment (file part + bss) and copy the file
        // bytes in; the rest stays zero.
        alloc_user_range(pgd, ph.p_vaddr, ph.p_memsz, flags);
        let file_end = (ph.p_offset + ph.p_filesz) as usize;
        if file_end > data.len() {
            return Err(ENOEXEC);
        }
        let seg = &data[ph.p_offset as usize..file_end];
        // SAFETY: the range was just mapped writable (we add _PAGE_RW while
        // loading if needed); segments are loaded before user code runs.
        unsafe {
            // Ensure the pages are writable for the copy even for RO/exec
            // segments by mapping with RW, then the final flags are already
            // what we set (RO segments get faults only on user writes, which
            // is fine; the copy here is a kernel write to the physical page
            // via the identity map, independent of the user PTE flags).
            copy_to_user_pgd(pgd, ph.p_vaddr, seg);
        }

        let seg_end = ph.p_vaddr + ph.p_memsz;
        brk = brk.max((seg_end + PAGE_SIZE - 1) & !(PAGE_SIZE - 1));
    }

    // Build the user stack: argv/envp/auxv, as create_elf_tables() does.
    let user_sp = setup_user_stack(pgd, &ehdr, argv0);

    Ok(LoadedElf {
        pgd,
        entry: ehdr.e_entry,
        user_sp,
        brk,
    })
}

/// `create_elf_tables()`: lay out the initial user stack.
///
/// From the top down: a NUL-terminated argv[0] string, then the auxv, envp
/// (empty) and argv arrays, then argc, all 8-byte aligned so the stack
/// pointer is 16-byte aligned at entry (the System V ABI).
fn setup_user_stack(pgd: u64, ehdr: &Elf64Ehdr, argv0: &[u8]) -> u64 {
    let stack_flags = _PAGE_PRESENT | _PAGE_RW | _PAGE_USER | _PAGE_NX;
    alloc_user_range(
        pgd,
        USER_STACK_TOP - USER_STACK_SIZE,
        USER_STACK_SIZE,
        stack_flags,
    );

    // Copy argv[0] near the top.
    let mut name = argv0.to_vec();
    name.push(0);
    let arg_addr = (USER_STACK_TOP - name.len() as u64) & !0xf;
    // SAFETY: the stack range was just mapped writable.
    unsafe { copy_to_user_pgd(pgd, arg_addr, &name) };

    // The auxv: AT_ENTRY, AT_PHDR info, AT_PAGESZ, AT_NULL.
    let auxv: [(u64, u64); 4] = [
        (AT_PAGESZ, PAGE_SIZE),
        (AT_ENTRY, ehdr.e_entry),
        (AT_PHNUM, ehdr.e_phnum as u64),
        (AT_NULL, 0),
    ];

    // Words laid out from the top of the used region downward:
    //   argc, argv[0], argv NULL, envp NULL, auxv...
    let mut words: Vec<u64> = alloc::vec![
        1,        // argc
        arg_addr, // argv[0]
        0,        // argv terminator
        0,        // envp terminator
    ];
    for (k, v) in auxv {
        words.push(k);
        words.push(v);
    }

    // Place the words so the final rsp is 16-byte aligned.
    let bytes = words.len() * 8;
    let mut sp = (arg_addr - bytes as u64) & !0xf;
    // Keep argc at the lowest address (where the ABI expects rsp).
    let base = sp;
    for (i, w) in words.iter().enumerate() {
        // SAFETY: within the mapped, writable stack region.
        let buf = w.to_le_bytes();
        unsafe { copy_to_user_pgd(pgd, base + (i * 8) as u64, &buf) };
    }
    sp = base;
    sp
}

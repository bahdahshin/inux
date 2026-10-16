// SPDX-License-Identifier: GPL-2.0 WITH Linux-syscall-note
//! ELF64 program-loading definitions.
//!
//! Ported from the 64-bit parts of `include/uapi/linux/elf.h`. Only what the
//! loader in [`crate::fs::binfmt_elf`] needs: the file and program headers
//! and the constants to pick out `PT_LOAD` segments.

/// `e_ident` magic: 0x7f 'E' 'L' 'F'.
pub const ELFMAG: &[u8; 4] = b"\x7fELF";
/// `EI_CLASS` = `ELFCLASS64`.
pub const ELFCLASS64: u8 = 2;
/// `EI_DATA` = `ELFDATA2LSB` (little endian).
pub const ELFDATA2LSB: u8 = 1;

// e_type
pub const ET_EXEC: u16 = 2;
pub const ET_DYN: u16 = 3;

// e_machine
pub const EM_X86_64: u16 = 62;

// p_type
pub const PT_LOAD: u32 = 1;
pub const PT_INTERP: u32 = 3;

// p_flags
pub const PF_X: u32 = 1;
pub const PF_W: u32 = 2;
pub const PF_R: u32 = 4;

// Auxiliary vector types (`auxv`).
pub const AT_NULL: u64 = 0;
pub const AT_PHDR: u64 = 3;
pub const AT_PHENT: u64 = 4;
pub const AT_PHNUM: u64 = 5;
pub const AT_PAGESZ: u64 = 6;
pub const AT_ENTRY: u64 = 9;

/// `Elf64_Ehdr`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Elf64Ehdr {
    pub e_ident: [u8; 16],
    pub e_type: u16,
    pub e_machine: u16,
    pub e_version: u32,
    pub e_entry: u64,
    pub e_phoff: u64,
    pub e_shoff: u64,
    pub e_flags: u32,
    pub e_ehsize: u16,
    pub e_phentsize: u16,
    pub e_phnum: u16,
    pub e_shentsize: u16,
    pub e_shnum: u16,
    pub e_shstrndx: u16,
}

/// `Elf64_Phdr`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Elf64Phdr {
    pub p_type: u32,
    pub p_flags: u32,
    pub p_offset: u64,
    pub p_vaddr: u64,
    pub p_paddr: u64,
    pub p_filesz: u64,
    pub p_memsz: u64,
    pub p_align: u64,
}

const _: () = assert!(core::mem::size_of::<Elf64Ehdr>() == 64);
const _: () = assert!(core::mem::size_of::<Elf64Phdr>() == 56);

/// Reads a `T` from `buf` at `off`, if it fits.
pub fn read_at<T: Copy>(buf: &[u8], off: usize) -> Option<T> {
    if off + core::mem::size_of::<T>() > buf.len() {
        return None;
    }
    // SAFETY: the bounds check above guarantees the read is in range; T is
    // Copy and #[repr(C)] with no invalid bit patterns for our POD headers.
    Some(unsafe { (buf.as_ptr().add(off) as *const T).read_unaligned() })
}

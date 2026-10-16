// SPDX-License-Identifier: GPL-2.0
//! The multiboot2 boot information.
//!
//! Linux on x86 is booted through its own boot protocol
//! (`Documentation/arch/x86/boot.rst`) and receives `struct boot_params`.
//! rinux is booted by GRUB's `multiboot2` command instead, which works the
//! same way under BIOS and UEFI. This module reads what Linux would find in
//! `boot_params`: the command line, the memory map and the text screen.
//! It is new code, written from the Multiboot2 specification.

const TAG_END: u32 = 0;
const TAG_CMDLINE: u32 = 1;
const TAG_BOOT_LOADER_NAME: u32 = 2;
const TAG_MODULE: u32 = 3;
const TAG_MMAP: u32 = 6;
const TAG_FRAMEBUFFER: u32 = 8;

/// `framebuffer_type` for EGA text mode.
pub const FRAMEBUFFER_TYPE_EGA_TEXT: u8 = 2;

/// The boot information structure GRUB passes in EBX.
#[derive(Clone, Copy)]
pub struct BootInfo {
    bytes: &'static [u8],
}

/// One memory map entry. The types are the e820 types.
#[derive(Clone, Copy, Debug)]
pub struct MmapEntry {
    pub base_addr: u64,
    pub length: u64,
    pub typ: u32,
}

/// The framebuffer info tag.
#[derive(Clone, Copy, Debug)]
pub struct Framebuffer {
    pub addr: u64,
    pub pitch: u32,
    pub width: u32,
    pub height: u32,
    pub bpp: u8,
    pub typ: u8,
}

/// A boot module, such as an initramfs.
#[derive(Clone, Copy, Debug)]
pub struct Module {
    pub start: u32,
    pub end: u32,
    pub cmdline: &'static [u8],
}

fn u32_at(b: &[u8], off: usize) -> u32 {
    u32::from_le_bytes(b[off..off + 4].try_into().unwrap())
}

fn u64_at(b: &[u8], off: usize) -> u64 {
    u64::from_le_bytes(b[off..off + 8].try_into().unwrap())
}

/// A NUL-terminated string field, without the NUL.
fn cstr(b: &[u8]) -> &[u8] {
    &b[..rinux_lib::string::strlen(b)]
}

impl BootInfo {
    /// Wraps the boot information at physical address `addr`.
    ///
    /// # Safety
    ///
    /// `addr` must be the address GRUB passed in EBX, identity-mapped and
    /// left untouched for the life of the kernel.
    pub unsafe fn from_addr(addr: usize) -> BootInfo {
        // SAFETY: the caller guarantees addr points at the boot information,
        // whose first field is its total size.
        let total_size = unsafe { (addr as *const u32).read() } as usize;
        // SAFETY: GRUB guarantees total_size bytes are readable there.
        let bytes = unsafe { core::slice::from_raw_parts(addr as *const u8, total_size) };
        BootInfo { bytes }
    }

    /// Iterates over `(type, tag bytes)`, the bytes including the header.
    fn tags(&self) -> impl Iterator<Item = (u32, &'static [u8])> {
        let bytes = self.bytes;
        let mut off = 8;
        core::iter::from_fn(move || {
            if off + 8 > bytes.len() {
                return None;
            }
            let typ = u32_at(bytes, off);
            let size = u32_at(bytes, off + 4) as usize;
            if typ == TAG_END || size < 8 || off + size > bytes.len() {
                return None;
            }
            let tag = &bytes[off..off + size];
            off += size.next_multiple_of(8);
            Some((typ, tag))
        })
    }

    fn tag(&self, want: u32) -> Option<&'static [u8]> {
        self.tags()
            .find(|&(typ, _)| typ == want)
            .map(|(_, tag)| tag)
    }

    /// The kernel command line.
    pub fn cmdline(&self) -> Option<&'static [u8]> {
        self.tag(TAG_CMDLINE).map(|t| cstr(&t[8..]))
    }

    /// The boot loader's name, such as "GRUB 2.12".
    pub fn boot_loader_name(&self) -> Option<&'static [u8]> {
        self.tag(TAG_BOOT_LOADER_NAME).map(|t| cstr(&t[8..]))
    }

    /// The memory map, in the order the firmware reported it.
    pub fn mmap(&self) -> impl Iterator<Item = MmapEntry> {
        let tag = self.tag(TAG_MMAP).unwrap_or(&[]);
        let entry_size = if tag.len() >= 16 {
            u32_at(tag, 8) as usize
        } else {
            0
        };
        let entries = if entry_size >= 20 { &tag[16..] } else { &[] };
        entries.chunks_exact(entry_size.max(1)).map(|e| MmapEntry {
            base_addr: u64_at(e, 0),
            length: u64_at(e, 8),
            typ: u32_at(e, 16),
        })
    }

    /// The framebuffer GRUB left the screen in, if it reported one.
    pub fn framebuffer(&self) -> Option<Framebuffer> {
        let t = self.tag(TAG_FRAMEBUFFER)?;
        if t.len() < 30 {
            return None;
        }
        Some(Framebuffer {
            addr: u64_at(t, 8),
            pitch: u32_at(t, 16),
            width: u32_at(t, 20),
            height: u32_at(t, 24),
            bpp: t[28],
            typ: t[29],
        })
    }

    /// Boot modules, in the order GRUB loaded them.
    pub fn modules(&self) -> impl Iterator<Item = Module> {
        self.tags()
            .filter(|&(typ, t)| typ == TAG_MODULE && t.len() >= 16)
            .map(|(_, t)| Module {
                start: u32_at(t, 8),
                end: u32_at(t, 12),
                cmdline: cstr(&t[16..]),
            })
    }
}

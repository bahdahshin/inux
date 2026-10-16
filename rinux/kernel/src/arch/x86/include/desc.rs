// SPDX-License-Identifier: GPL-2.0
//! Segment and gate descriptors.
//!
//! Ported from `arch/x86/include/asm/desc_defs.h` and the 64-bit parts of
//! `arch/x86/include/asm/desc.h`.

use core::arch::asm;

// _DESC_* building blocks.
const _DESC_ACCESSED: u16 = 0x0001;
const _DESC_DATA_WRITABLE: u16 = 0x0002;
const _DESC_CODE_READABLE: u16 = 0x0002;
const _DESC_CODE_EXECUTABLE: u16 = 0x0008;
const _DESC_S: u16 = 0x0010;
const fn _desc_dpl(dpl: u16) -> u16 {
    dpl << 5
}
const _DESC_PRESENT: u16 = 0x0080;
const _DESC_LONG_CODE: u16 = 0x2000;
const _DESC_DB: u16 = 0x4000;
const _DESC_GRANULARITY_4K: u16 = 0x8000;

const _DESC_DATA: u16 = _DESC_S | _DESC_PRESENT | _DESC_ACCESSED | _DESC_DATA_WRITABLE;
const _DESC_CODE: u16 =
    _DESC_S | _DESC_PRESENT | _DESC_ACCESSED | _DESC_CODE_READABLE | _DESC_CODE_EXECUTABLE;

pub const DESC_DATA32: u16 = _DESC_DATA | _DESC_GRANULARITY_4K | _DESC_DB;
pub const DESC_CODE32: u16 = _DESC_CODE | _DESC_GRANULARITY_4K | _DESC_DB;
pub const DESC_DATA64: u16 = _DESC_DATA | _DESC_GRANULARITY_4K | _DESC_DB;
pub const DESC_CODE64: u16 = _DESC_CODE | _DESC_GRANULARITY_4K | _DESC_LONG_CODE;
pub const DESC_USER: u16 = _desc_dpl(3);

// Gate types.
pub const GATE_INTERRUPT: u16 = 0xe;
pub const GATE_TRAP: u16 = 0xf;
pub const GATE_CALL: u16 = 0xc;
pub const GATE_TASK: u16 = 0x5;

// System descriptor types.
pub const DESC_TSS: u32 = 0x9;
pub const DESC_LDT: u32 = 0x2;

/// `IDT_ENTRIES`.
pub const IDT_ENTRIES: usize = 256;

/// `struct desc_struct`: an 8-byte segment descriptor, as one `u64`.
pub type DescStruct = u64;

/// `GDT_ENTRY_INIT(flags, base, limit)`.
pub const fn gdt_entry_init(flags: u16, base: u32, limit: u32) -> DescStruct {
    let flags = flags as u64;
    let base = base as u64;
    let limit = limit as u64;
    (limit & 0xffff)
        | (base & 0xffff) << 16
        | ((base >> 16) & 0xff) << 32
        | (flags & 0xff) << 40
        | ((limit >> 16) & 0xf) << 48
        | ((flags >> 12) & 0xf) << 52
        | ((base >> 24) & 0xff) << 56
}

/// `struct idt_bits`: IST index, gate type, DPL and present bit.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(transparent)]
pub struct IdtBits(pub u16);

impl IdtBits {
    pub const fn new(ist: u16, typ: u16, dpl: u16, p: bool) -> IdtBits {
        IdtBits((ist & 0x7) | (typ & 0x1f) << 8 | (dpl & 0x3) << 13 | (p as u16) << 15)
    }
}

/// `struct gate_struct` (`gate_desc`): a 16-byte IDT entry.
#[repr(C, packed)]
#[derive(Clone, Copy, Debug, Default)]
pub struct GateStruct {
    pub offset_low: u16,
    pub segment: u16,
    pub bits: IdtBits,
    pub offset_middle: u16,
    pub offset_high: u32,
    pub reserved: u32,
}

const _: () = assert!(core::mem::size_of::<GateStruct>() == 16);

impl GateStruct {
    /// `gate_offset()`.
    pub fn offset(&self) -> u64 {
        self.offset_low as u64 | (self.offset_middle as u64) << 16 | (self.offset_high as u64) << 32
    }
}

/// `struct idt_data`: one entry of an IDT setup table.
#[derive(Clone, Copy)]
pub struct IdtData {
    pub vector: u32,
    pub segment: u16,
    pub bits: IdtBits,
    pub addr: unsafe extern "C" fn(),
}

/// `idt_init_desc()`.
pub fn idt_init_desc(d: &IdtData) -> GateStruct {
    let addr = d.addr as usize as u64;
    GateStruct {
        offset_low: addr as u16,
        segment: d.segment,
        bits: d.bits,
        offset_middle: (addr >> 16) as u16,
        offset_high: (addr >> 32) as u32,
        reserved: 0,
    }
}

/// `struct ldttss_desc`: a 16-byte TSS or LDT descriptor, as two `u64`s.
pub type LdtTssDesc = [u64; 2];

/// `set_tssldt_descriptor()`.
pub const fn set_tssldt_descriptor(addr: u64, typ: u32, size: u32) -> LdtTssDesc {
    let size = size as u64;
    let low = (size & 0xffff)
        | (addr & 0xffff) << 16
        | ((addr >> 16) & 0xff) << 32
        | (typ as u64 & 0x1f) << 40
        | 1 << 47 // p
        | ((size >> 16) & 0xf) << 48
        | ((addr >> 24) & 0xff) << 56;
    [low, addr >> 32]
}

/// `struct desc_ptr`: the operand of LGDT, LIDT, SGDT and SIDT.
#[repr(C, packed)]
#[derive(Clone, Copy, Debug, Default)]
pub struct DescPtr {
    pub size: u16,
    pub address: u64,
}

/// `native_load_gdt()`.
///
/// # Safety
///
/// `dtr` must describe a valid GDT that outlives its use.
#[inline]
pub unsafe fn load_gdt(dtr: &DescPtr) {
    // SAFETY: the caller guarantees the GDT is valid.
    unsafe {
        asm!("lgdt ({})", in(reg) dtr, options(att_syntax, readonly, nostack, preserves_flags))
    }
}

/// `native_load_idt()`.
///
/// # Safety
///
/// `dtr` must describe a valid IDT that outlives its use.
#[inline]
pub unsafe fn load_idt(dtr: &DescPtr) {
    // SAFETY: the caller guarantees the IDT is valid.
    unsafe {
        asm!("lidt ({})", in(reg) dtr, options(att_syntax, readonly, nostack, preserves_flags))
    }
}

/// `store_idt()`.
#[inline]
pub fn store_idt() -> DescPtr {
    let mut dtr = DescPtr::default();
    // SAFETY: SIDT only writes the 10-byte operand.
    unsafe { asm!("sidt ({})", in(reg) &mut dtr, options(att_syntax, nostack, preserves_flags)) }
    dtr
}

/// `native_store_gdt()`.
#[inline]
pub fn store_gdt() -> DescPtr {
    let mut dtr = DescPtr::default();
    // SAFETY: SGDT only writes the 10-byte operand.
    unsafe { asm!("sgdt ({})", in(reg) &mut dtr, options(att_syntax, nostack, preserves_flags)) }
    dtr
}

/// `native_load_tr_desc()`: loads the task register.
///
/// # Safety
///
/// `sel` must select an available TSS descriptor in the current GDT.
#[inline]
pub unsafe fn load_tr_desc(sel: u16) {
    // SAFETY: the caller guarantees the selector is a valid TSS.
    unsafe { asm!("ltr {0:x}", in(reg) sel, options(nostack, preserves_flags)) }
}

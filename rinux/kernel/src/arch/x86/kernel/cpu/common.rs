// SPDX-License-Identifier: GPL-2.0-only
//! The GDT, the TSS with its interrupt stacks, and CPU identification.
//!
//! A partial port of `arch/x86/kernel/cpu/common.c`, with the exception
//! stack layout of `arch/x86/include/asm/cpu_entry_area.h` and
//! `page_64_types.h`. Linux maps the TSS and stacks into the per-CPU entry
//! area; rinux has one CPU and no kernel page tables of its own yet, so
//! they are plain statics.
//!
//! Not yet ported: feature flags beyond the basics, per-vendor setup,
//! errata and mitigations, microcode, and secondary CPUs.

use core::arch::asm;
use core::fmt::Write;
use core::mem::size_of;

use crate::arch::x86::include::desc::{
    DESC_CODE32, DESC_CODE64, DESC_DATA64, DESC_TSS, DESC_USER, DescPtr, DescStruct,
    gdt_entry_init, load_gdt, load_tr_desc, set_tssldt_descriptor,
};
use crate::arch::x86::include::processor::{
    __KERNEL_TSS_LIMIT, IO_BITMAP_OFFSET_INVALID, X86HwTss, cpuid,
};
use crate::arch::x86::include::segment::{
    __KERNEL_CS, GDT_ENTRIES, GDT_ENTRY_DEFAULT_USER_CS, GDT_ENTRY_DEFAULT_USER_DS,
    GDT_ENTRY_DEFAULT_USER32_CS, GDT_ENTRY_KERNEL_CS, GDT_ENTRY_KERNEL_DS, GDT_ENTRY_KERNEL32_CS,
    GDT_ENTRY_TSS,
};
use crate::arch::x86::kernel::idt::load_current_idt;
use crate::kernel::locking::{RacyCell, SpinLock};
use crate::kernel::printk::FmtBuf;

/// `struct gdt_page`.
#[repr(C, align(4096))]
pub struct GdtPage {
    pub gdt: [DescStruct; GDT_ENTRIES as usize],
}

const fn initial_gdt() -> [DescStruct; GDT_ENTRIES as usize] {
    let mut gdt = [0; GDT_ENTRIES as usize];
    // We need valid kernel segments for data and code in long mode too;
    // IRET checks the segment types. SYSRET also mandates this layout.
    gdt[GDT_ENTRY_KERNEL32_CS as usize] = gdt_entry_init(DESC_CODE32, 0, 0xfffff);
    gdt[GDT_ENTRY_KERNEL_CS as usize] = gdt_entry_init(DESC_CODE64, 0, 0xfffff);
    gdt[GDT_ENTRY_KERNEL_DS as usize] = gdt_entry_init(DESC_DATA64, 0, 0xfffff);
    gdt[GDT_ENTRY_DEFAULT_USER32_CS as usize] = gdt_entry_init(DESC_CODE32 | DESC_USER, 0, 0xfffff);
    gdt[GDT_ENTRY_DEFAULT_USER_DS as usize] = gdt_entry_init(DESC_DATA64 | DESC_USER, 0, 0xfffff);
    gdt[GDT_ENTRY_DEFAULT_USER_CS as usize] = gdt_entry_init(DESC_CODE64 | DESC_USER, 0, 0xfffff);
    gdt
}

/// `gdt_page`: the boot CPU's GDT.
pub static GDT_PAGE: RacyCell<GdtPage> = RacyCell::new(GdtPage { gdt: initial_gdt() });

/// `GDT_SIZE`.
const GDT_SIZE: usize = GDT_ENTRIES as usize * size_of::<DescStruct>();

// arch/x86/include/asm/page_64_types.h
pub const IST_INDEX_DF: usize = 0;
pub const IST_INDEX_NMI: usize = 1;
pub const IST_INDEX_DB: usize = 2;
pub const IST_INDEX_MCE: usize = 3;

/// `EXCEPTION_STKSZ`. Linux uses one page; the Rust oops path formats
/// into stack buffers, so rinux gives each IST stack 16 KiB.
pub const EXCEPTION_STKSZ: usize = 16 * 1024;

/// `struct exception_stacks`: the IST stacks, in `enum exception_stack_ordering`.
#[repr(C, align(4096))]
pub struct ExceptionStacks {
    pub df: [u8; EXCEPTION_STKSZ],
    pub nmi: [u8; EXCEPTION_STKSZ],
    pub db: [u8; EXCEPTION_STKSZ],
    pub mce: [u8; EXCEPTION_STKSZ],
}

pub static EXCEPTION_STACKS: RacyCell<ExceptionStacks> = RacyCell::new(ExceptionStacks {
    df: [0; EXCEPTION_STKSZ],
    nmi: [0; EXCEPTION_STKSZ],
    db: [0; EXCEPTION_STKSZ],
    mce: [0; EXCEPTION_STKSZ],
});

/// `cpu_tss_rw`: the boot CPU's TSS.
pub static CPU_TSS_RW: RacyCell<X86HwTss> = RacyCell::new(X86HwTss {
    reserved1: 0,
    sp0: 0,
    sp1: 0,
    sp2: 0,
    reserved2: 0,
    ist: [0; 7],
    reserved3: 0,
    reserved4: 0,
    reserved5: 0,
    io_bitmap_base: 0,
});

/// The bounds of each IST stack, for the unwinder: (name, bottom, top).
pub fn exception_stacks() -> [(&'static str, usize, usize); 4] {
    let s = EXCEPTION_STACKS.get() as usize;
    let range = |i: usize| (s + i * EXCEPTION_STKSZ, s + (i + 1) * EXCEPTION_STKSZ);
    let (df, nmi, db, mce) = (range(0), range(1), range(2), range(3));
    [
        ("#DF", df.0, df.1),
        ("NMI", nmi.0, nmi.1),
        ("#DB", db.0, db.1),
        ("#MC", mce.0, mce.1),
    ]
}

/// `load_direct_gdt()` plus the far return to `__KERNEL_CS` that Linux's
/// `startup_64` does after loading `gdt_page`: switch from the boot GDT of
/// `head_64` to the kernel's.
pub fn load_direct_gdt() {
    let gdt_descr = DescPtr {
        size: (GDT_SIZE - 1) as u16,
        address: GDT_PAGE.get() as u64,
    };
    // SAFETY: GDT_PAGE is a static GDT with the kernel code segment at the
    // same selector as the boot GDT, so the far return lands correctly.
    unsafe {
        load_gdt(&gdt_descr);
        asm!(
            "pushq ${cs}",
            "leaq 2f(%rip), %rax",
            "pushq %rax",
            "lretq",
            "2:",
            cs = const __KERNEL_CS,
            out("rax") _,
            options(att_syntax, preserves_flags),
        );
    }
}

/// `tss_setup_ist()`: points each IST slot at the top of its stack.
fn tss_setup_ist(tss: &mut X86HwTss) {
    let stacks = exception_stacks();
    let mut ist = tss.ist;
    ist[IST_INDEX_DF] = stacks[0].2 as u64;
    ist[IST_INDEX_NMI] = stacks[1].2 as u64;
    ist[IST_INDEX_DB] = stacks[2].2 as u64;
    ist[IST_INDEX_MCE] = stacks[3].2 as u64;
    tss.ist = ist;
}

/// `__set_tss_desc()`.
fn set_tss_desc(addr: *const X86HwTss) {
    let tss = set_tssldt_descriptor(addr as u64, DESC_TSS, __KERNEL_TSS_LIMIT);
    // SAFETY: boot CPU, interrupts off; the TSS descriptor takes two GDT
    // slots.
    unsafe {
        let gdt = &mut (*GDT_PAGE.get()).gdt;
        gdt[GDT_ENTRY_TSS as usize] = tss[0];
        gdt[GDT_ENTRY_TSS as usize + 1] = tss[1];
    }
}

/// `set_tss_sp0()`: the stack the CPU switches to on a ring 3 -> ring 0
/// trap (set for the current task before entering user mode and on every
/// context switch).
pub fn set_tss_sp0(top: u64) {
    // SAFETY: the TSS is a static the boot CPU owns; sp0 is read by hardware
    // only on privilege-level changes.
    unsafe { (*CPU_TSS_RW.get()).sp0 = top };
}

/// `cpu_init_exception_handling()`: sets up the TSS with its IST stacks,
/// loads the task register, and loads the IDT.
pub fn cpu_init_exception_handling() {
    // SAFETY: boot CPU, interrupts off, nothing else uses the TSS yet.
    let tss = unsafe { &mut *CPU_TSS_RW.get() };
    tss_setup_ist(tss);
    tss.io_bitmap_base = IO_BITMAP_OFFSET_INVALID;
    set_tss_desc(tss);
    // SAFETY: the GDT now has an available TSS descriptor at GDT_ENTRY_TSS.
    unsafe { load_tr_desc(GDT_ENTRY_TSS * 8) };
    load_current_idt();
}

// CPU identification.

pub const X86_VENDOR_INTEL: u8 = 0;
pub const X86_VENDOR_CYRIX: u8 = 1;
pub const X86_VENDOR_AMD: u8 = 2;
pub const X86_VENDOR_UMC: u8 = 3;
pub const X86_VENDOR_CENTAUR: u8 = 5;
pub const X86_VENDOR_TRANSMETA: u8 = 7;
pub const X86_VENDOR_NSC: u8 = 8;
pub const X86_VENDOR_HYGON: u8 = 9;
pub const X86_VENDOR_ZHAOXIN: u8 = 10;
pub const X86_VENDOR_VORTEX: u8 = 11;
pub const X86_VENDOR_UNKNOWN: u8 = 0xff;

/// The vendor strings and names of the `struct cpu_dev`s in
/// `arch/x86/kernel/cpu/*.c`.
const CPU_DEVS: &[(u8, &[u8; 12], &str)] = &[
    (X86_VENDOR_INTEL, b"GenuineIntel", "Intel"),
    (X86_VENDOR_AMD, b"AuthenticAMD", "AMD"),
    (X86_VENDOR_AMD, b"AMD ISBETTER", "AMD"),
    (X86_VENDOR_HYGON, b"HygonGenuine", "Hygon"),
    (X86_VENDOR_CENTAUR, b"CentaurHauls", "Centaur"),
    (X86_VENDOR_ZHAOXIN, b"  Shanghai  ", "zhaoxin"),
    (X86_VENDOR_CYRIX, b"CyrixInstead", "Cyrix"),
    (X86_VENDOR_TRANSMETA, b"GenuineTMx86", "Transmeta"),
    (X86_VENDOR_TRANSMETA, b"TransmetaCPU", "Transmeta"),
    (X86_VENDOR_NSC, b"Geode by NSC", "NSC"),
    (X86_VENDOR_UMC, b"UMC UMC UMC ", "UMC"),
    (X86_VENDOR_VORTEX, b"Vortex86 SoC", "Vortex"),
];

/// `X86_FEATURE_TSC`: CPUID leaf 1 EDX bit 4.
pub const X86_FEATURE_TSC: u32 = 4;

/// `struct cpuinfo_x86`, the parts used so far.
#[derive(Clone, Copy)]
pub struct CpuinfoX86 {
    /// CPU family.
    pub x86: u32,
    pub x86_vendor: u8,
    pub x86_model: u32,
    pub x86_stepping: u32,
    /// Maximum standard CPUID leaf, or -1 without CPUID.
    pub cpuid_level: i32,
    pub extended_cpuid_level: u32,
    pub x86_vendor_id: [u8; 16],
    pub x86_model_id: [u8; 64],
    /// CPUID leaf 1 EDX: word 0 of `x86_capability`.
    pub x86_capability0: u32,
}

impl CpuinfoX86 {
    const fn new() -> CpuinfoX86 {
        CpuinfoX86 {
            x86: 0,
            x86_vendor: X86_VENDOR_UNKNOWN,
            x86_model: 0,
            x86_stepping: 0,
            cpuid_level: -1,
            extended_cpuid_level: 0,
            x86_vendor_id: [0; 16],
            x86_model_id: [0; 64],
            x86_capability0: 0,
        }
    }

    /// `boot_cpu_has()` for word 0 features.
    pub fn has(&self, feature: u32) -> bool {
        self.x86_capability0 & (1 << feature) != 0
    }
}

/// `boot_cpu_data`.
pub static BOOT_CPU_DATA: SpinLock<CpuinfoX86> = SpinLock::new(CpuinfoX86::new());

/// `boot_cpu_data`, copied out.
pub fn boot_cpu_data() -> CpuinfoX86 {
    *BOOT_CPU_DATA.lock_irqsave()
}

/// `cpu_detect()`: vendor string, family, model and stepping.
fn cpu_detect(c: &mut CpuinfoX86) {
    let (max_std_leaf, b, cx, d) = cpuid(0);
    c.cpuid_level = max_std_leaf as i32;
    c.x86_vendor_id[0..4].copy_from_slice(&b.to_le_bytes());
    c.x86_vendor_id[4..8].copy_from_slice(&d.to_le_bytes());
    c.x86_vendor_id[8..12].copy_from_slice(&cx.to_le_bytes());

    c.x86 = 4;
    if c.cpuid_level >= 1 {
        let (sig, _, _, edx) = cpuid(1);
        c.x86 = x86_family(sig);
        c.x86_model = x86_model(sig);
        c.x86_stepping = sig & 0xf;
        c.x86_capability0 = edx;
    }
}

/// `x86_family()` from `arch/x86/lib/cpu.c`.
pub fn x86_family(sig: u32) -> u32 {
    let family = (sig >> 8) & 0xf;
    if family == 0xf {
        family + ((sig >> 20) & 0xff)
    } else {
        family
    }
}

/// `x86_model()` from `arch/x86/lib/cpu.c`.
pub fn x86_model(sig: u32) -> u32 {
    let model = (sig >> 4) & 0xf;
    if x86_family(sig) >= 0x6 {
        model + (((sig >> 16) & 0xf) << 4)
    } else {
        model
    }
}

/// `get_cpu_vendor()`.
fn get_cpu_vendor(c: &mut CpuinfoX86) {
    c.x86_vendor = CPU_DEVS
        .iter()
        .find(|(_, ident, _)| &c.x86_vendor_id[..12] == *ident)
        .map_or(X86_VENDOR_UNKNOWN, |(v, _, _)| *v);
}

/// `get_model_name()`: the brand string from CPUID 0x80000002-4, trimmed.
fn get_model_name(c: &mut CpuinfoX86) {
    if c.extended_cpuid_level < 0x8000_0004 {
        return;
    }
    let mut raw = [0u8; 49];
    for (i, leaf) in (0x8000_0002u32..=0x8000_0004).enumerate() {
        let (a, b, cx, d) = cpuid(leaf);
        for (j, reg) in [a, b, cx, d].into_iter().enumerate() {
            raw[i * 16 + j * 4..i * 16 + j * 4 + 4].copy_from_slice(&reg.to_le_bytes());
        }
    }
    // Trim leading spaces and trailing white space.
    let text = &raw[..rinux_lib::string::strlen(&raw)];
    let start = text.iter().position(|&b| b != b' ').unwrap_or(text.len());
    let text = &text[start..];
    let end = text
        .iter()
        .rposition(|&b| !rinux_lib::ctype::isspace(b))
        .map_or(0, |i| i + 1);
    c.x86_model_id = [0; 64];
    c.x86_model_id[..end].copy_from_slice(&text[..end]);
}

/// `early_cpu_init()`/`early_identify_cpu()`: identify the boot CPU.
pub fn early_cpu_init() {
    let mut c = CpuinfoX86::new();
    cpu_detect(&mut c);
    get_cpu_vendor(&mut c);
    c.extended_cpuid_level = cpuid(0x8000_0000).0;
    get_model_name(&mut c);
    *BOOT_CPU_DATA.lock_irqsave() = c;
}

/// `print_cpu_info()`, with the "CPU: " that the non-SMP caller prints.
pub fn print_cpu_info(c: &CpuinfoX86) {
    let mut line = FmtBuf::<160>::new();
    let model_id = &c.x86_model_id[..rinux_lib::string::strlen(&c.x86_model_id)];
    let vendor: Option<&[u8]> = match CPU_DEVS.iter().find(|(v, _, _)| *v == c.x86_vendor) {
        Some((_, _, name)) => Some(name.as_bytes()),
        None if c.cpuid_level >= 0 => {
            Some(&c.x86_vendor_id[..rinux_lib::string::strlen(&c.x86_vendor_id)])
        }
        None => None,
    };
    if let Some(vendor) = vendor
        && rinux_lib::string::strstr(model_id, vendor).is_none()
    {
        let _ = write!(line, "{} ", crate::kernel::printk::BStr(vendor));
    }
    if !model_id.is_empty() {
        let _ = write!(line, "{}", crate::kernel::printk::BStr(model_id));
    } else {
        let _ = write!(line, "{}86", c.x86);
    }
    let _ = write!(line, " (family: {:#x}, model: {:#x}", c.x86, c.x86_model);
    let _ = write!(line, ", stepping: {:#x})", c.x86_stepping);
    pr_info!("CPU: {}", line);
}

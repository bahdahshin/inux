// SPDX-License-Identifier: GPL-2.0
//! Architecture setup.
//!
//! A partial port of `setup_arch()` from `arch/x86/kernel/setup.c`:
//! identify the CPU, install the early exception gates, read the firmware
//! memory map, hand back the command line, parse early parameters (which
//! is where early consoles come up), and calibrate the TSC if a fast method
//! works. ACPI, DMI, memblock and the kernel page tables come later.
//!
//! Copyright (C) 1995  Linus Torvalds

use core::sync::atomic::{AtomicU32, Ordering};

use rinux_lib::string::strscpy;

use super::cpu::common::early_cpu_init;
use super::e820::{e820__memblock_setup, e820__memory_setup};
use super::idt::{idt_setup_early_pf, idt_setup_early_traps};
use super::tsc::tsc_early_init;
use crate::arch::x86::boot::head_64::MULTIBOOT2_BOOTLOADER_MAGIC;
use crate::arch::x86::boot::multiboot2::BootInfo;
use crate::init::main::{BOOT_COMMAND_LINE, COMMAND_LINE_SIZE, parse_early_param};

/// EAX at entry: the multiboot2 magic if a multiboot2 loader booted us.
pub static MULTIBOOT2_MAGIC: AtomicU32 = AtomicU32::new(0);
/// EBX at entry: the physical address of the boot information.
pub static MULTIBOOT2_INFO: AtomicU32 = AtomicU32::new(0);

/// The boot information, if a multiboot2 loader passed some. This is what
/// `boot_params` is to Linux.
pub fn boot_info() -> Option<BootInfo> {
    if MULTIBOOT2_MAGIC.load(Ordering::Relaxed) != MULTIBOOT2_BOOTLOADER_MAGIC {
        return None;
    }
    let addr = MULTIBOOT2_INFO.load(Ordering::Relaxed) as usize;
    // SAFETY: with the right magic, EBX held the boot information address,
    // which is below 4 GiB and so identity-mapped, and nothing reuses that
    // memory yet.
    Some(unsafe { BootInfo::from_addr(addr) })
}

/// `setup_arch()`: copies the command line into `command_line`.
pub fn setup_arch(command_line: &mut [u8; COMMAND_LINE_SIZE]) {
    early_cpu_init();
    idt_setup_early_traps();

    e820__memory_setup();

    let _ = strscpy(command_line, &*BOOT_COMMAND_LINE.lock());

    parse_early_param();

    idt_setup_early_pf();

    // Fill memblock from the e820 map and build the kernel page tables with
    // a direct map of RAM (page 0 left unmapped so NULL faults).
    e820__memblock_setup();
    crate::arch::x86::mm::init::init_mem_mapping();

    tsc_early_init();

    if boot_info().is_none() {
        pr_err!(
            "rinux: not booted by a multiboot2 loader (magic {:#x}); no command line or memory map",
            MULTIBOOT2_MAGIC.load(Ordering::Relaxed)
        );
    }
}

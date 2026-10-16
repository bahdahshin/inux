// SPDX-License-Identifier: GPL-2.0
//! The first Rust code on the boot CPU.
//!
//! A partial port of `arch/x86/kernel/head64.c`. Linux's
//! `x86_64_start_kernel()` resets the early page tables, clears BSS and
//! copies `boot_params` and the command line before calling
//! `start_kernel()`. GRUB already zeroed BSS when it loaded the ELF image,
//! and the boot page tables from `startup_32` stay in use, so what is left
//! is switching from the boot GDT to `gdt_page` (which Linux's
//! `startup_64` does) and `copy_bootdata()`.

use core::sync::atomic::Ordering;

use rinux_lib::string::strscpy;

use super::cpu::common::load_direct_gdt;
use super::setup::{MULTIBOOT2_INFO, MULTIBOOT2_MAGIC, boot_info};
use crate::init::main::{BOOT_COMMAND_LINE, start_kernel};

/// `copy_bootdata()`: saves the boot information and copies the command
/// line into `boot_command_line`.
fn copy_bootdata(magic: u32, info: u32) {
    MULTIBOOT2_MAGIC.store(magic, Ordering::Relaxed);
    MULTIBOOT2_INFO.store(info, Ordering::Relaxed);
    if let Some(cmdline) = boot_info().and_then(|bi| bi.cmdline()) {
        let _ = strscpy(&mut *BOOT_COMMAND_LINE.lock(), cmdline);
    }
}

/// `x86_64_start_kernel()`: called from `startup_64` with the multiboot2
/// magic and the address of the boot information.
pub extern "C" fn x86_64_start_kernel(magic: u32, info: u32) -> ! {
    load_direct_gdt();
    copy_bootdata(magic, info);
    start_kernel()
}

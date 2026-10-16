// SPDX-License-Identifier: GPL-2.0
//! rinux: the kernel image.
//!
//! The boot path mirrors Linux on x86-64:
//!
//! 1. GRUB loads this ELF with the multiboot2 protocol and enters
//!    `startup_32` in 32-bit protected mode ([`arch::x86::boot::head_64`]).
//! 2. `startup_32` switches to 64-bit mode and calls
//!    `x86_64_start_kernel()` ([`arch::x86::kernel::head64`]).
//! 3. That calls `start_kernel()` ([`init::main`]), which sets up the
//!    architecture, parses early parameters and brings up early consoles.
//!
//! Module paths follow the Linux source tree: `arch::x86::kernel::setup` is
//! `arch/x86/kernel/setup.c`, `kernel::printk` is `kernel/printk/`.

#![no_std]
#![no_main]
// Much of each ported file's API is only used by later steps.
#![allow(dead_code)]

extern crate alloc;

#[macro_use]
mod kernel;

mod arch;
mod block;
mod drivers;
mod fs;
mod include;
mod init;
mod mm;
mod net;
mod rinux;

/// The kernel's global allocator, backed by kmalloc (see [`mm::rust_alloc`]).
#[global_allocator]
static ALLOCATOR: mm::rust_alloc::KernelAllocator = mm::rust_alloc::KernelAllocator;

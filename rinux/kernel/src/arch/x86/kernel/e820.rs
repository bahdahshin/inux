// SPDX-License-Identifier: GPL-2.0
//! The firmware memory map.
//!
//! A partial port of `arch/x86/kernel/e820.c`: build the table from the
//! boot loader's map and print it. Sanitizing overlapping entries, and
//! handing the map to memblock, come with memory management.

use core::fmt;

use super::setup::boot_info;
use crate::kernel::locking::SpinLock;

// enum e820_type, arch/x86/include/asm/e820/types.h
pub const E820_TYPE_RAM: u32 = 1;
pub const E820_TYPE_RESERVED: u32 = 2;
pub const E820_TYPE_ACPI: u32 = 3;
pub const E820_TYPE_NVS: u32 = 4;
pub const E820_TYPE_UNUSABLE: u32 = 5;
pub const E820_TYPE_PMEM: u32 = 7;
pub const E820_TYPE_PRAM: u32 = 12;
pub const E820_TYPE_SOFT_RESERVED: u32 = 0xefff_ffff;

/// `E820_MAX_ENTRIES` without NUMA.
pub const E820_MAX_ENTRIES: usize = 128;

/// `FW_BUG` from `include/linux/printk.h`.
const FW_BUG: &str = "[Firmware Bug]: ";

/// `struct e820_entry`.
#[derive(Clone, Copy, Debug, Default)]
pub struct E820Entry {
    pub addr: u64,
    pub size: u64,
    pub typ: u32,
}

/// `struct e820_table`.
pub struct E820Table {
    pub nr_entries: usize,
    pub entries: [E820Entry; E820_MAX_ENTRIES],
}

/// `e820_table`: the map the kernel uses.
pub static E820_TABLE: SpinLock<E820Table> = SpinLock::new(E820Table {
    nr_entries: 0,
    entries: [E820Entry {
        addr: 0,
        size: 0,
        typ: 0,
    }; E820_MAX_ENTRIES],
});

/// `e820_print_type()`, as text for one line.
struct E820Type(u32);

impl fmt::Display for E820Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            E820_TYPE_RAM => f.write_str(" System RAM"),
            E820_TYPE_RESERVED => f.write_str(" device reserved"),
            E820_TYPE_SOFT_RESERVED => f.write_str(" soft reserved"),
            E820_TYPE_ACPI => f.write_str(" ACPI data"),
            E820_TYPE_NVS => f.write_str(" ACPI NVS"),
            E820_TYPE_UNUSABLE => f.write_str(" unusable"),
            E820_TYPE_PMEM | E820_TYPE_PRAM => write!(f, " persistent RAM (type {})", self.0),
            t => write!(f, " type {t}"),
        }
    }
}

/// `e820__print_table()`.
#[allow(non_snake_case)]
pub fn e820__print_table(who: &str) {
    let table = E820_TABLE.lock();
    let mut range_end_prev = 0u64;

    for entry in &table.entries[..table.nr_entries] {
        let range_start = entry.addr;
        let range_end = entry.addr.wrapping_add(entry.size);

        // Out of order E820 maps should not happen.
        if range_start < range_end_prev {
            pr_info!("{FW_BUG}out of order E820 entry!");
        }

        if range_start > range_end_prev {
            pr_info!(
                "{}: [gap {:#018x}-{:#018x}]",
                who,
                range_end_prev,
                range_start - 1
            );
        }

        // C prints "] " and then pr_cont()s the type, which starts with a
        // space of its own.
        pr_info!(
            "{}: [mem {:#018x}-{:#018x}] {}",
            who,
            range_start,
            range_end.wrapping_sub(1),
            E820Type(entry.typ)
        );

        range_end_prev = range_end;
    }
}

/// `e820__memory_setup()`: copies the boot loader's memory map into
/// `e820_table` and prints it.
#[allow(non_snake_case)]
pub fn e820__memory_setup() {
    let Some(bi) = boot_info() else { return };
    {
        let mut table = E820_TABLE.lock();
        let mut n = 0;
        for e in bi.mmap().take(E820_MAX_ENTRIES) {
            table.entries[n] = E820Entry {
                addr: e.base_addr,
                size: e.length,
                typ: e.typ,
            };
            n += 1;
        }
        table.nr_entries = n;
    }
    // Linux names the source here; the default is "BIOS-e820".
    pr_info!("BIOS-provided physical RAM map:");
    e820__print_table("multiboot2");
}

/// `e820__memblock_setup()`: feed the e820 RAM ranges to memblock and
/// reserve the low 1 MiB and the kernel image, as `setup_arch()` does with
/// `memblock_x86_fill()` and `reserve_*`.
#[allow(non_snake_case)]
pub fn e820__memblock_setup() {
    use crate::mm::memblock::{memblock_add, memblock_reserve};

    let table = E820_TABLE.lock();
    for entry in &table.entries[..table.nr_entries] {
        if entry.typ == E820_TYPE_RAM {
            memblock_add(entry.addr, entry.size);
        }
    }
    drop(table);

    // The low 1 MiB holds the BIOS/boot data and real-mode trampolines.
    memblock_reserve(0, 0x100000);

    // The kernel image, from the linker symbols.
    unsafe extern "C" {
        static _text: u8;
        static _end: u8;
    }
    let text = &raw const _text as u64;
    let end = &raw const _end as u64;
    memblock_reserve(text, end - text);

    // Boot-loader modules (the initramfs), as `reserve_initrd()` does. They
    // must survive until unpacked and must not be handed to the allocators.
    if let Some(bi) = crate::arch::x86::kernel::setup::boot_info() {
        for m in bi.modules() {
            memblock_reserve(m.start as u64, (m.end - m.start) as u64);
        }
    }
}

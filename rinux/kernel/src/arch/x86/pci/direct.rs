// SPDX-License-Identifier: GPL-2.0
//! Low-level direct PCI config space access (type 1).
//!
//! A port of the type-1 accessors in `arch/x86/pci/direct.c`: a config read
//! or write is an address written to port `0xCF8` followed by a data
//! transfer through `0xCFC`, done under a lock so the address/data pair is
//! atomic. The type-2 (pre-PCI-2.0) accessors and the MMCONFIG path are not
//! ported.

use crate::arch::x86::include::io::{inb, inl, inw, outb, outl, outw};
use crate::kernel::locking::SpinLock;

/// `pci_config_lock`: serializes the two-step CF8/CFC accesses.
static PCI_CONFIG_LOCK: SpinLock<()> = SpinLock::new(());

/// `PCI_CONF1_ADDRESS(bus, devfn, reg)`: the value written to `0xCF8`.
///
/// Bit 31 enables the cycle; bits 11:8 of an extended register go in the
/// high nibble, the bus and devfn in 23:16 and 15:8, and the register's low
/// byte (dword-aligned) in 7:0.
#[inline]
fn conf1_address(bus: u8, devfn: u8, reg: u16) -> u32 {
    0x8000_0000
        | ((reg as u32 & 0xf00) << 16)
        | ((bus as u32) << 16)
        | ((devfn as u32) << 8)
        | (reg as u32 & 0xfc)
}

/// `pci_conf1_read()`: read `len` (1, 2 or 4) bytes of config register `reg`
/// of `bus:devfn`. Returns all-ones if the slot is empty, as the hardware
/// does.
pub fn pci_conf1_read(bus: u8, devfn: u8, reg: u16, len: u8) -> u32 {
    let _guard = PCI_CONFIG_LOCK.lock_irqsave();
    // SAFETY: CF8/CFC are the PCI config mechanism-1 ports; we own them for
    // the duration of the lock, and the data port read has no other effect.
    unsafe {
        outl(conf1_address(bus, devfn, reg), 0xcf8);
        match len {
            1 => inb(0xcfc + (reg & 3)) as u32,
            2 => inw(0xcfc + (reg & 2)) as u32,
            _ => inl(0xcfc),
        }
    }
}

/// `pci_conf1_write()`: write the low `len` bytes of `value` to config
/// register `reg` of `bus:devfn`.
pub fn pci_conf1_write(bus: u8, devfn: u8, reg: u16, len: u8, value: u32) {
    let _guard = PCI_CONFIG_LOCK.lock_irqsave();
    // SAFETY: as in pci_conf1_read; a config write only configures the device.
    unsafe {
        outl(conf1_address(bus, devfn, reg), 0xcf8);
        match len {
            1 => outb(value as u8, 0xcfc + (reg & 3)),
            2 => outw(value as u16, 0xcfc + (reg & 2)),
            _ => outl(value, 0xcfc),
        }
    }
}

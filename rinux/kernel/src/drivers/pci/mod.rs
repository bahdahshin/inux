// SPDX-License-Identifier: GPL-2.0
//! The PCI bus: config-space accessors and a device enumeration.
//!
//! A small port of the PCI core (`drivers/pci/`): the `PCI_DEVFN`/`PCI_SLOT`/
//! `PCI_FUNC` helpers and the config accessors from `include/linux/pci.h`,
//! and a depth-first bus scan like `pci_scan_child_bus()`/`pci_scan_slot()`
//! in `drivers/pci/probe.c`. It finds every function on bus 0 (and any bus
//! behind a PCI-to-PCI bridge) and records the identity header.
//!
//! Not ported: the driver model (`struct pci_driver`, probe/binding), BAR
//! sizing and resource assignment, capability and PCIe extended-config
//! parsing, MSI/MSI-X, and hotplug. Devices are only enumerated, not driven.

use alloc::vec::Vec;

use crate::arch::x86::pci::direct::{pci_conf1_read, pci_conf1_write};
use crate::include::uapi::pci_regs::*;

/// `PCI_DEVFN(slot, func)`: pack a slot (0..32) and function (0..8).
#[inline]
pub const fn pci_devfn(slot: u8, func: u8) -> u8 {
    ((slot & 0x1f) << 3) | (func & 0x07)
}

/// `PCI_SLOT(devfn)`.
#[inline]
pub const fn pci_slot(devfn: u8) -> u8 {
    (devfn >> 3) & 0x1f
}

/// `PCI_FUNC(devfn)`.
#[inline]
pub const fn pci_func(devfn: u8) -> u8 {
    devfn & 0x07
}

/// An enumerated PCI function (`struct pci_dev`, reduced to its identity).
pub struct PciDev {
    pub bus: u8,
    pub devfn: u8,
    pub vendor: u16,
    pub device: u16,
    /// 24-bit class code: base class, subclass and programming interface.
    pub class: u32,
    pub revision: u8,
    pub header_type: u8,
    /// The wired IRQ (`PCI_INTERRUPT_LINE`), or 0 if none.
    pub irq: u8,
    /// Raw base address registers (header type 0 only; zero otherwise).
    pub bars: [u32; 6],
}

impl PciDev {
    /// `pci_read_config_byte()`.
    pub fn read_config_byte(bus: u8, devfn: u8, reg: u16) -> u8 {
        pci_conf1_read(bus, devfn, reg, 1) as u8
    }

    /// `pci_read_config_word()`.
    pub fn read_config_word(bus: u8, devfn: u8, reg: u16) -> u16 {
        pci_conf1_read(bus, devfn, reg, 2) as u16
    }

    /// `pci_read_config_dword()`.
    pub fn read_config_dword(bus: u8, devfn: u8, reg: u16) -> u32 {
        pci_conf1_read(bus, devfn, reg, 4)
    }

    /// `pci_write_config_word()`.
    pub fn write_config_word(bus: u8, devfn: u8, reg: u16, value: u16) {
        pci_conf1_write(bus, devfn, reg, 2, value as u32);
    }

    /// The base class code (`class >> 16`).
    pub fn base_class(&self) -> u8 {
        (self.class >> 16) as u8
    }

    /// A short name for the base class, for reporting.
    pub fn class_name(&self) -> &'static str {
        match self.base_class() {
            PCI_BASE_CLASS_STORAGE => "storage",
            PCI_BASE_CLASS_NETWORK => "network",
            PCI_BASE_CLASS_DISPLAY => "display",
            PCI_BASE_CLASS_MULTIMEDIA => "multimedia",
            PCI_BASE_CLASS_MEMORY => "memory",
            PCI_BASE_CLASS_BRIDGE => "bridge",
            PCI_BASE_CLASS_COMMUNICATION => "communication",
            PCI_BASE_CLASS_SYSTEM => "system",
            PCI_BASE_CLASS_SERIAL => "serial-bus",
            _ => "other",
        }
    }

    /// Decodes BAR `i`: `(address, is_io)`. A 64-bit memory BAR's high half
    /// is ignored here (the low 32 bits are enough for identification).
    pub fn bar_address(&self, i: usize) -> (u32, bool) {
        let raw = self.bars[i];
        if raw & PCI_BASE_ADDRESS_SPACE_IO != 0 {
            (raw & PCI_BASE_ADDRESS_IO_MASK, true)
        } else {
            (raw & PCI_BASE_ADDRESS_MEM_MASK, false)
        }
    }
}

/// Reads the identity header of `bus:devfn`, or `None` if the slot is empty.
fn read_dev(bus: u8, devfn: u8) -> Option<PciDev> {
    let vendor = PciDev::read_config_word(bus, devfn, PCI_VENDOR_ID);
    // 0xffff (and the all-ones a missing slot returns) means no device.
    if vendor == 0xffff {
        return None;
    }
    let device = PciDev::read_config_word(bus, devfn, PCI_DEVICE_ID);
    let class = PciDev::read_config_dword(bus, devfn, PCI_CLASS_REVISION) >> 8;
    let revision = PciDev::read_config_byte(bus, devfn, PCI_REVISION_ID);
    let header_type = PciDev::read_config_byte(bus, devfn, PCI_HEADER_TYPE);
    let irq = PciDev::read_config_byte(bus, devfn, PCI_INTERRUPT_LINE);

    let mut bars = [0u32; 6];
    if header_type & PCI_HEADER_TYPE_MASK == PCI_HEADER_TYPE_NORMAL {
        for (i, bar) in bars.iter_mut().enumerate() {
            *bar = PciDev::read_config_dword(bus, devfn, PCI_BASE_ADDRESS_0 + (i as u16) * 4);
        }
    }

    Some(PciDev {
        bus,
        devfn,
        vendor,
        device,
        class,
        revision,
        header_type,
        irq,
        bars,
    })
}

/// `pci_scan_slot()`: scan one slot's functions into `out`, recursing behind
/// any bridge. Function 0 must exist; functions 1..8 are scanned only if it
/// marks the device multi-function.
fn scan_slot(bus: u8, slot: u8, out: &mut Vec<PciDev>) {
    let dev0 = match read_dev(bus, pci_devfn(slot, 0)) {
        Some(d) => d,
        None => return,
    };
    let last_func = if dev0.header_type & PCI_HEADER_TYPE_MFD != 0 {
        8
    } else {
        1
    };

    for func in 0..last_func {
        let dev = match read_dev(bus, pci_devfn(slot, func)) {
            Some(d) => d,
            None => continue,
        };
        // A PCI-to-PCI bridge: descend to its secondary bus.
        let is_bridge = dev.header_type & PCI_HEADER_TYPE_MASK == PCI_HEADER_TYPE_BRIDGE;
        let secondary = if is_bridge {
            Some(PciDev::read_config_byte(bus, dev.devfn, PCI_SECONDARY_BUS))
        } else {
            None
        };
        out.push(dev);
        // Descend, guarding against a bridge pointing at its own or an
        // upstream bus (which would loop).
        if let Some(sec) = secondary
            && sec > bus
        {
            scan_bus(sec, out);
        }
    }
}

/// `pci_scan_child_bus()`: scan every slot on `bus` into `out`.
fn scan_bus(bus: u8, out: &mut Vec<PciDev>) {
    for slot in 0..32u8 {
        scan_slot(bus, slot, out);
    }
}

/// `pci_scan_root_bus()`: enumerate the whole PCI hierarchy from bus 0.
pub fn scan_root() -> Vec<PciDev> {
    let mut out = Vec::new();
    scan_bus(0, &mut out);
    out
}

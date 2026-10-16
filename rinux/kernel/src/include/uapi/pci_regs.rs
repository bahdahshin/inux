// SPDX-License-Identifier: GPL-2.0 WITH Linux-syscall-note
//! PCI configuration space register offsets and bits.
//!
//! A subset of `include/uapi/linux/pci_regs.h`: the header-0/header-1 fields
//! the bus scan reads. The capability lists, PCIe extended config, and the
//! many device-specific registers are not ported yet.

/// Vendor ID (16 bits). `0xffff` means no device in the slot.
pub const PCI_VENDOR_ID: u16 = 0x00;
/// Device ID (16 bits).
pub const PCI_DEVICE_ID: u16 = 0x02;
/// Command register (16 bits).
pub const PCI_COMMAND: u16 = 0x04;
pub const PCI_COMMAND_IO: u16 = 0x1; // Enable response in I/O space
pub const PCI_COMMAND_MEMORY: u16 = 0x2; // Enable response in Memory space
pub const PCI_COMMAND_MASTER: u16 = 0x4; // Enable bus mastering
/// Status register (16 bits).
pub const PCI_STATUS: u16 = 0x06;
/// High 24 bits are the class code, low 8 the revision.
pub const PCI_CLASS_REVISION: u16 = 0x08;
/// Revision ID (8 bits).
pub const PCI_REVISION_ID: u16 = 0x08;
/// Register-level programming interface (8 bits).
pub const PCI_CLASS_PROG: u16 = 0x09;
/// Device class (16 bits: base class in the high byte, subclass in the low).
pub const PCI_CLASS_DEVICE: u16 = 0x0a;
/// Header type (8 bits).
pub const PCI_HEADER_TYPE: u16 = 0x0e;
pub const PCI_HEADER_TYPE_MASK: u8 = 0x7f;
pub const PCI_HEADER_TYPE_NORMAL: u8 = 0;
pub const PCI_HEADER_TYPE_BRIDGE: u8 = 1;
pub const PCI_HEADER_TYPE_CARDBUS: u8 = 2;
/// Multi-function device (the device may answer on functions 1..7).
pub const PCI_HEADER_TYPE_MFD: u8 = 0x80;

/// Base address registers (32 bits each). Header type 0 has six, header
/// type 1 (a bridge) has two.
pub const PCI_BASE_ADDRESS_0: u16 = 0x10;
pub const PCI_BASE_ADDRESS_1: u16 = 0x14;
pub const PCI_BASE_ADDRESS_2: u16 = 0x18;
pub const PCI_BASE_ADDRESS_3: u16 = 0x1c;
pub const PCI_BASE_ADDRESS_4: u16 = 0x20;
pub const PCI_BASE_ADDRESS_5: u16 = 0x24;
/// 0 = memory BAR, 1 = I/O BAR.
pub const PCI_BASE_ADDRESS_SPACE_IO: u32 = 0x01;
pub const PCI_BASE_ADDRESS_MEM_TYPE_MASK: u32 = 0x06;
pub const PCI_BASE_ADDRESS_MEM_TYPE_64: u32 = 0x04; // 64-bit address
pub const PCI_BASE_ADDRESS_MEM_MASK: u32 = !0x0f;
pub const PCI_BASE_ADDRESS_IO_MASK: u32 = !0x03;

/// Subsystem vendor ID (header type 0, 16 bits).
pub const PCI_SUBSYSTEM_VENDOR_ID: u16 = 0x2c;
/// Subsystem ID (header type 0, 16 bits).
pub const PCI_SUBSYSTEM_ID: u16 = 0x2e;
/// Interrupt line, i.e. the IRQ the device is wired to (8 bits).
pub const PCI_INTERRUPT_LINE: u16 = 0x3c;
/// Interrupt pin: which of INTA#..INTD# the device drives (8 bits).
pub const PCI_INTERRUPT_PIN: u16 = 0x3d;

// Header type 1 (PCI-to-PCI bridge) bus-number registers.
/// This bridge's primary (upstream) bus number (8 bits).
pub const PCI_PRIMARY_BUS: u16 = 0x18;
/// The bus number immediately behind the bridge (8 bits).
pub const PCI_SECONDARY_BUS: u16 = 0x19;
/// Highest bus number behind the bridge (8 bits).
pub const PCI_SUBORDINATE_BUS: u16 = 0x1a;

// Base class codes (`include/linux/pci_ids.h`, `PCI_BASE_CLASS_*`), for the
// scan's human-readable output.
pub const PCI_BASE_CLASS_STORAGE: u8 = 0x01;
pub const PCI_BASE_CLASS_NETWORK: u8 = 0x02;
pub const PCI_BASE_CLASS_DISPLAY: u8 = 0x03;
pub const PCI_BASE_CLASS_MULTIMEDIA: u8 = 0x04;
pub const PCI_BASE_CLASS_MEMORY: u8 = 0x05;
pub const PCI_BASE_CLASS_BRIDGE: u8 = 0x06;
pub const PCI_BASE_CLASS_COMMUNICATION: u8 = 0x07;
pub const PCI_BASE_CLASS_SYSTEM: u8 = 0x08;
pub const PCI_BASE_CLASS_SERIAL: u8 = 0x0c;

// SPDX-License-Identifier: GPL-2.0 WITH Linux-syscall-note
//! `include/uapi/linux`.

pub mod elf;
pub mod icmp;
pub mod if_arp;
pub mod if_ether;
#[path = "in.rs"]
pub mod in_;
pub mod ip;
pub mod pci_regs;
pub mod socket;
pub mod tcp;
pub mod udp;
pub mod virtio_blk;
pub mod virtio_config;
pub mod virtio_console;
pub mod virtio_ids;
pub mod virtio_net;
pub mod virtio_pci;
pub mod virtio_ring;

#[allow(dead_code, non_upper_case_globals, clippy::all)]
#[rustfmt::skip]
pub mod serial_reg;

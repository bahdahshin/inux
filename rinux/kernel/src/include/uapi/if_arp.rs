// SPDX-License-Identifier: GPL-2.0 WITH Linux-syscall-note
//! Address Resolution Protocol constants.
//!
//! A subset of `include/uapi/linux/if_arp.h`: the hardware type and the
//! request/reply opcodes. The `arphdr` fixed part is 8 bytes, followed by
//! the sender/target hardware and protocol addresses.

/// ARP hardware type: Ethernet.
pub const ARPHRD_ETHER: u16 = 1;

/// ARP opcode: request ("who has this IP?").
pub const ARPOP_REQUEST: u16 = 1;
/// ARP opcode: reply ("I have it, here is my MAC").
pub const ARPOP_REPLY: u16 = 2;

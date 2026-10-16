// SPDX-License-Identifier: GPL-2.0 WITH Linux-syscall-note
//! Ethernet (IEEE 802.3) constants.
//!
//! A subset of `include/uapi/linux/if_ether.h`: the address and header
//! lengths and the EtherType values the stack uses.

/// Octets in one Ethernet address.
pub const ETH_ALEN: usize = 6;
/// Total octets in an Ethernet header (dst + src + type).
pub const ETH_HLEN: usize = 14;
/// Minimum frame length, sans FCS (short frames are padded to this).
pub const ETH_ZLEN: usize = 60;
/// Maximum frame length, sans FCS.
pub const ETH_FRAME_LEN: usize = 1514;

/// EtherType: Internet Protocol v4.
pub const ETH_P_IP: u16 = 0x0800;
/// EtherType: Address Resolution Protocol.
pub const ETH_P_ARP: u16 = 0x0806;

/// The broadcast Ethernet address.
pub const ETH_BROADCAST: [u8; ETH_ALEN] = [0xff; ETH_ALEN];

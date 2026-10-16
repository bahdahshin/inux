// SPDX-License-Identifier: GPL-2.0
//! The networking stack, as in Linux's `net/`.
//!
//! So far: the link layer (Ethernet framing and ARP), the Internet checksum,
//! IPv4 with ICMP/UDP/TCP, and a small datagram socket on top. The generic
//! socket/sock split, the file-descriptor table and sockets for user space
//! come later.

pub mod arp;
pub mod checksum;
pub mod ethernet;
pub mod ipv4;
pub mod socket;

// SPDX-License-Identifier: GPL-2.0 WITH Linux-syscall-note
//! IP protocol numbers.
//!
//! A subset of `include/uapi/linux/in.h`: the `IPPROTO_*` values the stack
//! uses in the IPv4 header's protocol field.

/// Internet Control Message Protocol.
pub const IPPROTO_ICMP: u8 = 1;
/// Transmission Control Protocol.
pub const IPPROTO_TCP: u8 = 6;
/// User Datagram Protocol.
pub const IPPROTO_UDP: u8 = 17;

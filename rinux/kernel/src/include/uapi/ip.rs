// SPDX-License-Identifier: GPL-2.0 WITH Linux-syscall-note
//! IPv4 header constants.
//!
//! A subset of `include/uapi/linux/ip.h`. The `iphdr` struct uses C
//! bitfields for the version/IHL byte, so rinux builds and reads the fixed
//! 20-byte header field by field instead of mapping a struct.

/// IPv4 version number (the high nibble of byte 0).
pub const IPVERSION: u8 = 4;
/// Length of a header with no options, in 32-bit words (the low nibble).
pub const IP_IHL_NOOPTS: u8 = 5;
/// Length of a header with no options, in bytes.
pub const IP_HLEN: usize = 20;
/// A reasonable default time-to-live.
pub const IPDEFTTL: u8 = 64;

// Byte offsets of the fields within the fixed header.
pub const IP_OFF_VER_IHL: usize = 0;
pub const IP_OFF_TOS: usize = 1;
pub const IP_OFF_TOT_LEN: usize = 2;
pub const IP_OFF_ID: usize = 4;
pub const IP_OFF_FRAG_OFF: usize = 6;
pub const IP_OFF_TTL: usize = 8;
pub const IP_OFF_PROTOCOL: usize = 9;
pub const IP_OFF_CHECK: usize = 10;
pub const IP_OFF_SADDR: usize = 12;
pub const IP_OFF_DADDR: usize = 16;

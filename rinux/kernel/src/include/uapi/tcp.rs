// SPDX-License-Identifier: GPL-2.0 WITH Linux-syscall-note
//! TCP header constants.
//!
//! A subset of `include/uapi/linux/tcp.h` (the `tcphdr` field layout) and the
//! control-flag bits from `include/net/tcp.h` (`TCPHDR_*`). The C `tcphdr`
//! uses bitfields for the data-offset/flags word, so rinux builds and reads
//! the fixed 20-byte header field by field.

/// Length of a TCP header with no options.
pub const TCP_HLEN: usize = 20;

pub const TCP_OFF_SOURCE: usize = 0;
pub const TCP_OFF_DEST: usize = 2;
pub const TCP_OFF_SEQ: usize = 4;
pub const TCP_OFF_ACK_SEQ: usize = 8;
/// Byte 12: the high nibble is the data offset in 32-bit words.
pub const TCP_OFF_DOFF: usize = 12;
/// Byte 13: the control flags.
pub const TCP_OFF_FLAGS: usize = 13;
pub const TCP_OFF_WINDOW: usize = 14;
pub const TCP_OFF_CHECK: usize = 16;
pub const TCP_OFF_URG_PTR: usize = 18;

// Control-flag bits (`TCPHDR_*` in include/net/tcp.h).
pub const TCPHDR_FIN: u8 = 0x01;
pub const TCPHDR_SYN: u8 = 0x02;
pub const TCPHDR_RST: u8 = 0x04;
pub const TCPHDR_PSH: u8 = 0x08;
pub const TCPHDR_ACK: u8 = 0x10;
pub const TCPHDR_URG: u8 = 0x20;

// SPDX-License-Identifier: GPL-2.0
//! TCP segments, as in `net/ipv4/tcp_output.c` and `tcp_input.c`.
//!
//! A tiny port of the segment build/parse: lay down a 20-byte header with the
//! IPv4 pseudo-header checksum (`tcp_v4_send_check`/`__tcp_transmit_skb`), and
//! read the sequence numbers, flags and payload off a received segment
//! (`tcp_v4_rcv`). The connection state machine (`tcp_rcv_state_process`), the
//! socket layer, options, windows, congestion control and retransmission are
//! not here; the caller drives a lock-step active open over a reliable local
//! link (see the `check_tcp` boot check).

use crate::include::uapi::in_::IPPROTO_TCP;
use crate::include::uapi::tcp::*;
use crate::net::checksum::{csum_fold, csum_partial};

/// Writes the TCP header of `seg` (payload already in `seg[20..]`) and fills
/// in the pseudo-header checksum. `seg.len()` is the whole segment.
#[allow(clippy::too_many_arguments)]
pub fn build(
    seg: &mut [u8],
    src_port: u16,
    dst_port: u16,
    seq: u32,
    ack: u32,
    flags: u8,
    window: u16,
    src_ip: &[u8; 4],
    dst_ip: &[u8; 4],
) {
    seg[TCP_OFF_SOURCE..TCP_OFF_SOURCE + 2].copy_from_slice(&src_port.to_be_bytes());
    seg[TCP_OFF_DEST..TCP_OFF_DEST + 2].copy_from_slice(&dst_port.to_be_bytes());
    seg[TCP_OFF_SEQ..TCP_OFF_SEQ + 4].copy_from_slice(&seq.to_be_bytes());
    seg[TCP_OFF_ACK_SEQ..TCP_OFF_ACK_SEQ + 4].copy_from_slice(&ack.to_be_bytes());
    seg[TCP_OFF_DOFF] = (5u8) << 4; // data offset = 5 words, no options
    seg[TCP_OFF_FLAGS] = flags;
    seg[TCP_OFF_WINDOW..TCP_OFF_WINDOW + 2].copy_from_slice(&window.to_be_bytes());
    seg[TCP_OFF_CHECK..TCP_OFF_CHECK + 2].copy_from_slice(&[0, 0]);
    seg[TCP_OFF_URG_PTR..TCP_OFF_URG_PTR + 2].copy_from_slice(&[0, 0]);

    let total = seg.len() as u16;
    let mut sum = csum_partial(src_ip, 0);
    sum = csum_partial(dst_ip, sum);
    sum = csum_partial(&[0, IPPROTO_TCP], sum);
    sum = csum_partial(&total.to_be_bytes(), sum);
    sum = csum_partial(seg, sum);
    let check = csum_fold(sum);
    seg[TCP_OFF_CHECK..TCP_OFF_CHECK + 2].copy_from_slice(&check.to_be_bytes());
}

/// A parsed TCP segment.
pub struct TcpSegment<'a> {
    pub src_port: u16,
    pub dst_port: u16,
    pub seq: u32,
    pub ack: u32,
    pub flags: u8,
    pub window: u16,
    pub payload: &'a [u8],
}

/// Parses `seg` (the TCP part of an IPv4 packet). `seg.len()` must be the
/// segment length derived from the IP total length, so the payload is sized
/// correctly.
pub fn parse(seg: &[u8]) -> Option<TcpSegment<'_>> {
    if seg.len() < TCP_HLEN {
        return None;
    }
    let doff = (seg[TCP_OFF_DOFF] >> 4) as usize * 4;
    if doff < TCP_HLEN || doff > seg.len() {
        return None;
    }
    Some(TcpSegment {
        src_port: u16::from_be_bytes([seg[TCP_OFF_SOURCE], seg[TCP_OFF_SOURCE + 1]]),
        dst_port: u16::from_be_bytes([seg[TCP_OFF_DEST], seg[TCP_OFF_DEST + 1]]),
        seq: u32::from_be_bytes([seg[4], seg[5], seg[6], seg[7]]),
        ack: u32::from_be_bytes([seg[8], seg[9], seg[10], seg[11]]),
        flags: seg[TCP_OFF_FLAGS],
        window: u16::from_be_bytes([seg[TCP_OFF_WINDOW], seg[TCP_OFF_WINDOW + 1]]),
        payload: &seg[doff..],
    })
}

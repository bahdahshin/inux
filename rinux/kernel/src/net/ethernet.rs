// SPDX-License-Identifier: GPL-2.0
//! Ethernet framing.
//!
//! A tiny port of the header build/parse helpers of `net/ethernet/eth.c`:
//! lay down a 14-byte Ethernet header (`eth_header`) and read the EtherType
//! and source address off a received frame. No VLAN tags, no `eth_type_trans`
//! protocol classification beyond the raw type field.

use crate::include::uapi::if_ether::{ETH_ALEN, ETH_HLEN};

/// `eth_header()`: write the destination, source and EtherType into the
/// first 14 bytes of `buf`. Returns [`ETH_HLEN`].
pub fn eth_header(
    buf: &mut [u8],
    dst: &[u8; ETH_ALEN],
    src: &[u8; ETH_ALEN],
    ethertype: u16,
) -> usize {
    buf[..ETH_ALEN].copy_from_slice(dst);
    buf[ETH_ALEN..2 * ETH_ALEN].copy_from_slice(src);
    buf[2 * ETH_ALEN..ETH_HLEN].copy_from_slice(&ethertype.to_be_bytes());
    ETH_HLEN
}

/// The EtherType of a received frame.
pub fn eth_type(frame: &[u8]) -> Option<u16> {
    if frame.len() < ETH_HLEN {
        return None;
    }
    Some(u16::from_be_bytes([
        frame[2 * ETH_ALEN],
        frame[2 * ETH_ALEN + 1],
    ]))
}

/// The payload after the Ethernet header.
pub fn eth_payload(frame: &[u8]) -> &[u8] {
    &frame[ETH_HLEN.min(frame.len())..]
}

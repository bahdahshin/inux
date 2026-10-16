// SPDX-License-Identifier: GPL-2.0
//! The Address Resolution Protocol (IPv4 over Ethernet).
//!
//! A tiny port of the request/reply handling of `net/ipv4/arp.c`: build an
//! ARP request that asks for the MAC behind an IPv4 address (`arp_send`),
//! and parse an ARP reply (`arp_rcv`/`arp_process`). Only Ethernet/IPv4 ARP
//! is handled; there is no ARP cache, no gratuitous ARP and no proxying.

use crate::include::uapi::if_arp::{ARPHRD_ETHER, ARPOP_REPLY, ARPOP_REQUEST};
use crate::include::uapi::if_ether::{ETH_ALEN, ETH_BROADCAST, ETH_HLEN, ETH_P_ARP, ETH_P_IP};
use crate::net::ethernet::{eth_header, eth_payload, eth_type};

/// Length of an Ethernet/IPv4 ARP packet: 8-byte fixed header plus two
/// 6-byte MACs and two 4-byte IPs.
pub const ARP_ETH_IP_LEN: usize = 8 + 2 * ETH_ALEN + 2 * 4;
/// A full ARP frame: Ethernet header plus the ARP packet.
pub const ARP_FRAME_LEN: usize = ETH_HLEN + ARP_ETH_IP_LEN;

/// Builds a broadcast ARP request into `buf`, asking who owns `target_ip`.
/// Returns the total Ethernet frame length.
pub fn build_request(
    buf: &mut [u8],
    src_mac: &[u8; ETH_ALEN],
    src_ip: &[u8; 4],
    target_ip: &[u8; 4],
) -> usize {
    let n = eth_header(buf, &ETH_BROADCAST, src_mac, ETH_P_ARP);
    let p = &mut buf[n..n + ARP_ETH_IP_LEN];
    p[0..2].copy_from_slice(&ARPHRD_ETHER.to_be_bytes()); // hardware type
    p[2..4].copy_from_slice(&ETH_P_IP.to_be_bytes()); // protocol type
    p[4] = ETH_ALEN as u8; // hardware address length
    p[5] = 4; // protocol address length
    p[6..8].copy_from_slice(&ARPOP_REQUEST.to_be_bytes());
    p[8..14].copy_from_slice(src_mac); // sender hardware address
    p[14..18].copy_from_slice(src_ip); // sender protocol address
    p[18..24].copy_from_slice(&[0u8; ETH_ALEN]); // target hw addr (unknown)
    p[24..28].copy_from_slice(target_ip); // target protocol address
    n + ARP_ETH_IP_LEN
}

/// A parsed ARP reply: the owner's MAC and IPv4 address.
pub struct ArpReply {
    pub mac: [u8; ETH_ALEN],
    pub ip: [u8; 4],
}

/// Parses `frame` as an ARP reply, returning the sender's MAC and IP.
/// Returns `None` if it is not an Ethernet/IPv4 ARP reply.
pub fn parse_reply(frame: &[u8]) -> Option<ArpReply> {
    if eth_type(frame)? != ETH_P_ARP {
        return None;
    }
    let p = eth_payload(frame);
    if p.len() < ARP_ETH_IP_LEN {
        return None;
    }
    let htype = u16::from_be_bytes([p[0], p[1]]);
    let ptype = u16::from_be_bytes([p[2], p[3]]);
    let op = u16::from_be_bytes([p[6], p[7]]);
    if htype != ARPHRD_ETHER || ptype != ETH_P_IP || op != ARPOP_REPLY {
        return None;
    }
    let mut mac = [0u8; ETH_ALEN];
    mac.copy_from_slice(&p[8..14]);
    let mut ip = [0u8; 4];
    ip.copy_from_slice(&p[14..18]);
    Some(ArpReply { mac, ip })
}

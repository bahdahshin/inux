// SPDX-License-Identifier: GPL-2.0
//! Boot-time IP autoconfiguration (DHCP), as in `net/ipv4/ipconfig.c`.
//!
//! A small port of the DHCP client the kernel uses to configure an interface
//! at boot (for NFS root): build a BOOTP/DHCP message (`ic_dhcp_init_options`
//! and `ic_bootp_send_if`), and parse the server's reply (`ic_bootp_recv`),
//! pulling the offered address and the subnet/router/DNS options.
//!
//! Simplified from Linux's: DHCP only (no BOOTP-reply or RARP fallback), one
//! interface, and a fixed parameter request list; the retry/backoff state
//! machine lives in the caller.

/// BOOTP client and server UDP ports.
pub const BOOTP_CLIENT_PORT: u16 = 68;
pub const BOOTP_SERVER_PORT: u16 = 67;

// DHCP message types (option 53).
pub const DHCPDISCOVER: u8 = 1;
pub const DHCPOFFER: u8 = 2;
pub const DHCPREQUEST: u8 = 3;
pub const DHCPACK: u8 = 5;
pub const DHCPNAK: u8 = 6;

/// The RFC 1048 magic cookie that precedes the option area.
const DHCP_MAGIC: [u8; 4] = [99, 130, 83, 99];

// BOOTP op codes.
const BOOTREQUEST: u8 = 1;
const BOOTREPLY: u8 = 2;
const HTYPE_ETHER: u8 = 1;

/// Fixed BOOTP header length (op .. boot_file), before the options.
pub const BOOTP_FIXED_LEN: usize = 236;
/// Offset of the `yiaddr` (your-IP) field.
const BOOTP_OFF_YIADDR: usize = 16;
/// Offset of the `chaddr` (client hardware address) field.
const BOOTP_OFF_CHADDR: usize = 28;

/// Builds a DHCP message (BOOTP header + options) into `buf`, returning its
/// length. For a request, `server_ip` and `req_ip` identify the lease.
pub fn build_message(
    buf: &mut [u8],
    mac: &[u8; 6],
    xid: u32,
    msg_type: u8,
    server_ip: Option<[u8; 4]>,
    req_ip: Option<[u8; 4]>,
) -> usize {
    buf[..BOOTP_FIXED_LEN].fill(0);
    buf[0] = BOOTREQUEST;
    buf[1] = HTYPE_ETHER;
    buf[2] = 6; // hlen
    buf[4..8].copy_from_slice(&xid.to_be_bytes());
    // Ask the server to broadcast the reply (we have no IP to unicast to).
    buf[10..12].copy_from_slice(&0x8000u16.to_be_bytes());
    buf[BOOTP_OFF_CHADDR..BOOTP_OFF_CHADDR + 6].copy_from_slice(mac);

    // Options, after the magic cookie.
    let mut e = BOOTP_FIXED_LEN;
    buf[e..e + 4].copy_from_slice(&DHCP_MAGIC);
    e += 4;
    // Option 53: DHCP message type.
    buf[e] = 53;
    buf[e + 1] = 1;
    buf[e + 2] = msg_type;
    e += 3;
    if msg_type == DHCPREQUEST {
        if let Some(sid) = server_ip {
            buf[e] = 54; // server identifier
            buf[e + 1] = 4;
            buf[e + 2..e + 6].copy_from_slice(&sid);
            e += 6;
        }
        if let Some(ip) = req_ip {
            buf[e] = 50; // requested IP address
            buf[e + 1] = 4;
            buf[e + 2..e + 6].copy_from_slice(&ip);
            e += 6;
        }
    }
    // Option 55: parameter request list (subnet, router, DNS).
    buf[e] = 55;
    buf[e + 1] = 3;
    buf[e + 2] = 1; // subnet mask
    buf[e + 3] = 3; // router
    buf[e + 4] = 6; // DNS
    e += 5;
    buf[e] = 255; // end
    e += 1;
    e
}

/// The interesting fields of a parsed DHCP reply.
#[derive(Default)]
pub struct DhcpReply {
    pub msg_type: u8,
    pub your_ip: [u8; 4],
    pub server_id: [u8; 4],
    pub subnet: [u8; 4],
    pub router: [u8; 4],
    pub dns: [u8; 4],
}

/// `ic_bootp_recv()`: parse a BOOTP/DHCP reply for transaction `xid`.
pub fn parse_reply(buf: &[u8], xid: u32) -> Option<DhcpReply> {
    if buf.len() < BOOTP_FIXED_LEN + 4 || buf[0] != BOOTREPLY {
        return None;
    }
    if u32::from_be_bytes([buf[4], buf[5], buf[6], buf[7]]) != xid {
        return None;
    }
    if buf[BOOTP_FIXED_LEN..BOOTP_FIXED_LEN + 4] != DHCP_MAGIC {
        return None;
    }

    let mut r = DhcpReply::default();
    r.your_ip
        .copy_from_slice(&buf[BOOTP_OFF_YIADDR..BOOTP_OFF_YIADDR + 4]);

    // Walk the TLV options after the cookie.
    let mut i = BOOTP_FIXED_LEN + 4;
    while i < buf.len() {
        let code = buf[i];
        if code == 255 {
            break; // end
        }
        if code == 0 {
            i += 1; // pad
            continue;
        }
        if i + 1 >= buf.len() {
            break;
        }
        let len = buf[i + 1] as usize;
        let val = i + 2;
        if val + len > buf.len() {
            break;
        }
        match code {
            53 if len >= 1 => r.msg_type = buf[val],
            54 if len >= 4 => r.server_id.copy_from_slice(&buf[val..val + 4]),
            1 if len >= 4 => r.subnet.copy_from_slice(&buf[val..val + 4]),
            3 if len >= 4 => r.router.copy_from_slice(&buf[val..val + 4]),
            6 if len >= 4 => r.dns.copy_from_slice(&buf[val..val + 4]),
            _ => {}
        }
        i = val + len;
    }
    Some(r)
}

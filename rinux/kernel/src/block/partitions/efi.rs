// SPDX-License-Identifier: GPL-2.0
//! GUID Partition Table (GPT) parsing.
//!
//! A small port of `block/partitions/efi.c`: read the primary GPT header at
//! LBA 1 (signature "EFI PART"), validate its CRC-32, then read and parse the
//! partition-entry array, returning the used entries. The EFI CRC is the
//! standard (zlib) CRC-32, which the kernel computes as `crc32(~0, buf) ^ ~0`.
//!
//! Not ported: the backup/alternate header, repair logic, the many partition
//! type-GUID checks, and the MBR/GPT protective-MBR validation (the header is
//! read directly).

use alloc::vec::Vec;

use rinux_lib::crc::crc32_le;
use rinux_lib::errno::{EINVAL, Result};

use crate::block::BlockDevice;

const GPT_SIGNATURE: &[u8; 8] = b"EFI PART";

/// One used GPT partition.
pub struct GptPartition {
    pub first_lba: u64,
    pub last_lba: u64,
    /// The partition name, ASCII subset of its UTF-16LE label.
    pub name: Vec<u8>,
}

fn le16(b: &[u8], o: usize) -> u16 {
    u16::from_le_bytes([b[o], b[o + 1]])
}
fn le32(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
}
fn le64(b: &[u8], o: usize) -> u64 {
    let mut v = [0u8; 8];
    v.copy_from_slice(&b[o..o + 8]);
    u64::from_le_bytes(v)
}

/// `efi_crc32()`: the standard CRC-32 with the usual pre/post inversion.
fn efi_crc32(data: &[u8]) -> u32 {
    crc32_le(!0, data) ^ !0
}

/// The result of parsing a GPT: its partitions and whether the header CRC
/// validated.
pub struct Gpt {
    pub partitions: Vec<GptPartition>,
    pub header_crc_ok: bool,
}

/// Parses the primary GPT of `dev`. Fails with `EINVAL` if LBA 1 does not
/// hold a GPT header.
pub fn parse_gpt(dev: &dyn BlockDevice) -> Result<Gpt> {
    let mut hdr = [0u8; 512];
    dev.read(1, &mut hdr)?;
    if &hdr[0..8] != GPT_SIGNATURE {
        return Err(EINVAL);
    }

    let header_size = (le32(&hdr, 12) as usize).min(512);
    let stored_crc = le32(&hdr, 16);
    // The CRC is computed with its own field zeroed.
    let mut hcopy = hdr[..header_size].to_vec();
    hcopy[16..20].copy_from_slice(&[0, 0, 0, 0]);
    let header_crc_ok = efi_crc32(&hcopy) == stored_crc;

    let entry_lba = le64(&hdr, 72);
    let num_entries = le32(&hdr, 80) as usize;
    let entry_size = le32(&hdr, 84) as usize;
    if entry_size < 128 || num_entries == 0 || num_entries > 1024 {
        return Err(EINVAL);
    }

    // Read the whole entry array (rounded up to a sector boundary).
    let total = num_entries * entry_size;
    let sectors = total.div_ceil(512);
    let mut buf = alloc::vec![0u8; sectors * 512];
    dev.read(entry_lba, &mut buf)?;

    let mut partitions = Vec::new();
    for i in 0..num_entries {
        let e = i * entry_size;
        if e + entry_size > buf.len() {
            break;
        }
        // An unused entry has an all-zero type GUID.
        if buf[e..e + 16].iter().all(|&b| b == 0) {
            continue;
        }
        let first_lba = le64(&buf, e + 32);
        let last_lba = le64(&buf, e + 40);
        // The name is UTF-16LE in the 72 bytes at offset 56; keep the ASCII.
        let mut name = Vec::new();
        let mut j = e + 56;
        while j + 1 < e + 128 {
            let c = le16(&buf, j);
            if c == 0 {
                break;
            }
            if c < 0x80 {
                name.push(c as u8);
            }
            j += 2;
        }
        partitions.push(GptPartition {
            first_lba,
            last_lba,
            name,
        });
    }
    Ok(Gpt {
        partitions,
        header_crc_ok,
    })
}

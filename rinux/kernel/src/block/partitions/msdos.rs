// SPDX-License-Identifier: GPL-2.0
//! MS-DOS (MBR) partition tables.
//!
//! A small port of `block/partitions/msdos.c`: validate the 0x55AA boot
//! signature and read the four primary partition entries from the partition
//! table at byte 0x1be of the first sector. Each 16-byte `msdos_partition`
//! entry gives a boot flag, a system-id (type) byte, and the start LBA and
//! length in sectors.
//!
//! Not ported: extended/logical partitions, the many subformat probes
//! (Solaris, BSD, Minix, ...), and GPT (`efi.c`).

use alloc::vec::Vec;

use rinux_lib::errno::{EINVAL, Result};

use crate::block::BlockDevice;

/// `MSDOS_LABEL_MAGIC`: the two signature bytes at the end of the MBR.
const MSDOS_LABEL_MAGIC1: u8 = 0x55;
const MSDOS_LABEL_MAGIC2: u8 = 0xaa;
/// Offset of the partition table within the boot sector.
const PARTITION_TABLE_OFFSET: usize = 0x1be;

/// One primary MBR partition.
pub struct Partition {
    /// The boot flag was `0x80` (active).
    pub bootable: bool,
    /// The partition system id (`sys_ind`), e.g. 0x83 for Linux.
    pub sys_ind: u8,
    /// First sector (LBA).
    pub start_lba: u32,
    /// Length in sectors.
    pub nr_sectors: u32,
}

fn le32(b: &[u8], off: usize) -> u32 {
    u32::from_le_bytes([b[off], b[off + 1], b[off + 2], b[off + 3]])
}

/// Parses the MBR partition table of `dev`, returning its primary partitions.
/// Fails with `EINVAL` if the device has no valid DOS partition table.
pub fn parse_mbr(dev: &dyn BlockDevice) -> Result<Vec<Partition>> {
    let mut mbr = [0u8; 512];
    dev.read(0, &mut mbr)?;

    if mbr[510] != MSDOS_LABEL_MAGIC1 || mbr[511] != MSDOS_LABEL_MAGIC2 {
        return Err(EINVAL);
    }

    let mut out = Vec::new();
    for i in 0..4 {
        let p = PARTITION_TABLE_OFFSET + i * 16;
        let boot_ind = mbr[p];
        // A valid table has boot flags of only 0x00 or 0x80; anything else
        // means this sector is not really a partition table.
        if boot_ind != 0 && boot_ind != 0x80 {
            return Err(EINVAL);
        }
        let sys_ind = mbr[p + 4];
        if sys_ind == 0 {
            continue; // empty slot
        }
        out.push(Partition {
            bootable: boot_ind == 0x80,
            sys_ind,
            start_lba: le32(&mbr, p + 8),
            nr_sectors: le32(&mbr, p + 12),
        });
    }
    Ok(out)
}

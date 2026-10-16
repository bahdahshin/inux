// SPDX-License-Identifier: GPL-2.0
//! A read-only FAT32 filesystem.
//!
//! A small port of the read paths of `fs/fat/`: parse the BIOS parameter
//! block (`fat_fill_super` in `inode.c`), follow a cluster's chain through
//! the file allocation table (`fat_ent_read` in `fatent.c`), and walk the
//! 32-byte directory entries (`fs/fat/dir.c`) matching short 8.3 names. The
//! on-disk layout constants come from `include/uapi/linux/msdos_fs.h`.
//!
//! Simplified from Linux's: FAT32 only (not FAT12/16), read-only, short 8.3
//! names (VFAT long-name entries are skipped), and no FAT cache, timestamps
//! or `fsinfo`.

use alloc::boxed::Box;
use alloc::vec::Vec;

use rinux_lib::errno::{EINVAL, ENOENT, Result};

use crate::block::BlockDevice;

/// `ATTR_EXT`: the attribute byte of a VFAT long-name entry.
const ATTR_LFN: u8 = 0x0f;
/// `ATTR_VOLUME`: a volume-label entry, not a real file.
const ATTR_VOLUME: u8 = 0x08;
/// A deleted directory entry's first name byte.
const DELETED: u8 = 0xe5;
/// End-of-chain marker: FAT32 entries at or above this end a cluster chain.
const FAT32_EOC: u32 = 0x0fff_fff8;

fn le16(b: &[u8], off: usize) -> u16 {
    u16::from_le_bytes([b[off], b[off + 1]])
}
fn le32(b: &[u8], off: usize) -> u32 {
    u32::from_le_bytes([b[off], b[off + 1], b[off + 2], b[off + 3]])
}

/// A mounted FAT32 filesystem.
pub struct Fat32Fs {
    dev: Box<dyn BlockDevice>,
    bytes_per_sector: u32,
    sec_per_clus: u32,
    reserved: u32,
    fat_size: u32,
    root_cluster: u32,
    first_data_sector: u32,
}

/// One directory entry's identity.
pub struct FatDirEntry {
    /// The raw 11-byte 8.3 name (space padded, upper-cased).
    pub name: [u8; 11],
    pub attr: u8,
    pub first_cluster: u32,
    pub size: u32,
}

impl Fat32Fs {
    /// `fat_fill_super()`: read and validate the boot sector.
    pub fn mount(dev: Box<dyn BlockDevice>) -> Result<Fat32Fs> {
        let mut bs = [0u8; 512];
        dev.read(0, &mut bs)?;
        // Boot signature and a plausible FAT32 BPB.
        if bs[510] != 0x55 || bs[511] != 0xaa {
            return Err(EINVAL);
        }
        let bytes_per_sector = le16(&bs, 11) as u32;
        let sec_per_clus = bs[13] as u32;
        let reserved = le16(&bs, 14) as u32;
        let num_fats = bs[16] as u32;
        let fat16_len = le16(&bs, 22) as u32; // 0 for FAT32
        let fat_size = le32(&bs, 36); // sectors per FAT (FAT32)
        let root_cluster = le32(&bs, 44);
        if bytes_per_sector != 512
            || sec_per_clus == 0
            || num_fats == 0
            || fat16_len != 0
            || fat_size == 0
            || root_cluster < 2
        {
            return Err(EINVAL);
        }
        Ok(Fat32Fs {
            dev,
            bytes_per_sector,
            sec_per_clus,
            reserved,
            fat_size,
            root_cluster,
            first_data_sector: reserved + num_fats * fat_size,
        })
    }

    fn read_sector(&self, sector: u32) -> Result<Vec<u8>> {
        let bs = self.bytes_per_sector as usize;
        let mut buf = alloc::vec![0u8; bs];
        self.dev.read(sector as u64 * (bs as u64 / 512), &mut buf)?;
        Ok(buf)
    }

    fn cluster_first_sector(&self, cluster: u32) -> u32 {
        self.first_data_sector + (cluster - 2) * self.sec_per_clus
    }

    /// Reads a whole cluster's bytes.
    fn read_cluster(&self, cluster: u32) -> Result<Vec<u8>> {
        let start = self.cluster_first_sector(cluster);
        let mut out = Vec::with_capacity((self.sec_per_clus * self.bytes_per_sector) as usize);
        for i in 0..self.sec_per_clus {
            out.extend_from_slice(&self.read_sector(start + i)?);
        }
        Ok(out)
    }

    /// `fat_ent_read()`: the next cluster after `cluster` in the chain.
    fn fat_next(&self, cluster: u32) -> Result<u32> {
        let fat_offset = cluster * 4;
        let sector = self.reserved + fat_offset / self.bytes_per_sector;
        let off = (fat_offset % self.bytes_per_sector) as usize;
        let buf = self.read_sector(sector)?;
        Ok(le32(&buf, off) & 0x0fff_ffff)
    }

    /// Collects the cluster chain starting at `start` (bounded, to guard
    /// against a cyclic FAT).
    fn chain(&self, start: u32) -> Result<Vec<u32>> {
        let mut clusters = Vec::new();
        let mut c = start;
        while (2..FAT32_EOC).contains(&c) && clusters.len() < 1 << 20 {
            clusters.push(c);
            c = self.fat_next(c)?;
        }
        Ok(clusters)
    }

    /// Reads the raw bytes of a cluster chain starting at `start`.
    fn read_chain(&self, start: u32) -> Result<Vec<u8>> {
        let mut out = Vec::new();
        for c in self.chain(start)? {
            out.extend_from_slice(&self.read_cluster(c)?);
        }
        Ok(out)
    }

    /// Lists a directory (given by its first cluster), skipping long-name,
    /// volume-label, deleted and `.`/`..` entries.
    pub fn read_dir(&self, first_cluster: u32) -> Result<Vec<FatDirEntry>> {
        let data = self.read_chain(first_cluster)?;
        let mut out = Vec::new();
        let mut off = 0;
        while off + 32 <= data.len() {
            let e = &data[off..off + 32];
            off += 32;
            match e[0] {
                0x00 => break, // no more entries
                DELETED => continue,
                b'.' => continue, // . and ..
                _ => {}
            }
            let attr = e[11];
            if attr & ATTR_LFN == ATTR_LFN || attr & ATTR_VOLUME != 0 {
                continue;
            }
            let mut name = [0u8; 11];
            name.copy_from_slice(&e[0..11]);
            let first_cluster = ((le16(e, 20) as u32) << 16) | le16(e, 26) as u32;
            out.push(FatDirEntry {
                name,
                attr,
                first_cluster,
                size: le32(e, 28),
            });
        }
        Ok(out)
    }

    /// Lists the root directory.
    pub fn read_root(&self) -> Result<Vec<FatDirEntry>> {
        self.read_dir(self.root_cluster)
    }

    /// Reads a file in the root directory by its name (any case, 8.3).
    pub fn read_root_file(&self, name: &[u8]) -> Result<Vec<u8>> {
        let want = to_83(name).ok_or(EINVAL)?;
        for e in self.read_root()? {
            if e.name == want {
                let mut data = self.read_chain(e.first_cluster)?;
                data.truncate(e.size as usize);
                return Ok(data);
            }
        }
        Err(ENOENT)
    }
}

/// Converts a filename to the 11-byte 8.3 form (upper-cased, space padded),
/// or `None` if it does not fit 8.3.
fn to_83(name: &[u8]) -> Option<[u8; 11]> {
    let mut out = [b' '; 11];
    let (base, ext) = match name.iter().position(|&c| c == b'.') {
        Some(dot) => (&name[..dot], &name[dot + 1..]),
        None => (name, &b""[..]),
    };
    if base.is_empty() || base.len() > 8 || ext.len() > 3 {
        return None;
    }
    for (i, &c) in base.iter().enumerate() {
        out[i] = c.to_ascii_uppercase();
    }
    for (i, &c) in ext.iter().enumerate() {
        out[8 + i] = c.to_ascii_uppercase();
    }
    Some(out)
}

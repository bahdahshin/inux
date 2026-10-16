// SPDX-License-Identifier: GPL-2.0
//! A read-only ext4 filesystem.
//!
//! A small port of the read paths of `fs/ext4/super.c`, `fs/ext4/inode.c`,
//! `fs/ext4/extents.c` and `fs/ext4/dir.c`. ext4's on-disk superblock, group
//! descriptor, inode and directory entry are a superset of ext2's, so the
//! shared structures are reused from [`crate::fs::ext2::layout`]; the new
//! part is the extent tree (`ext4_ext_map_blocks`), which replaces ext2's
//! indirect block map when the inode carries [`EXT4_EXTENTS_FL`].
//!
//! Simplified from Linux's: read-only, extents and the classic indirect map
//! only (no inline data, no 64-bit group descriptors beyond the low 32 bits
//! of block numbers we use, no journal, checksums, htree or xattrs).

pub mod ext4_extents;

use alloc::boxed::Box;
use alloc::vec::Vec;

use rinux_lib::errno::{EINVAL, EIO, ENOENT, Result};

use crate::block::BlockDevice;
use crate::fs::ext2::layout::*;
use ext4_extents::*;

/// A mounted ext4 filesystem.
pub struct Ext4Fs {
    dev: Box<dyn BlockDevice>,
    block_size: u32,
    inodes_per_group: u32,
    inode_size: u32,
    gdt_block: u32,
}

impl Ext4Fs {
    /// Reads and validates the superblock (`ext4_fill_super`).
    pub fn mount(dev: Box<dyn BlockDevice>) -> Result<Ext4Fs> {
        let mut sb_buf = alloc::vec![0u8; 1024];
        dev.read(EXT2_SUPERBLOCK_OFFSET / 512, &mut sb_buf)?;
        let sb: Ext2SuperBlock = read_struct(&sb_buf, 0).ok_or(EIO)?;
        if sb.s_magic != EXT2_SUPER_MAGIC {
            return Err(EINVAL);
        }
        let block_size = 1024u32 << sb.s_log_block_size;
        let inode_size = if sb.s_rev_level >= 1 {
            sb.s_inode_size as u32
        } else {
            EXT2_GOOD_OLD_INODE_SIZE as u32
        };
        Ok(Ext4Fs {
            dev,
            block_size,
            inodes_per_group: sb.s_inodes_per_group,
            inode_size,
            gdt_block: sb.s_first_data_block + 1,
        })
    }

    fn read_block(&self, block: u32) -> Result<Vec<u8>> {
        let bs = self.block_size as usize;
        let mut buf = alloc::vec![0u8; bs];
        let sector = block as u64 * (bs as u64 / 512);
        self.dev.read(sector, &mut buf)?;
        Ok(buf)
    }

    fn read_group_desc(&self, group: u32) -> Result<Ext2GroupDesc> {
        // ext4 group descriptors can be 64 bytes, but the inode-table block
        // number is in the same low fields as ext2's 32-byte descriptor, and
        // mke2fs keeps 32-byte descriptors unless 64bit is set.
        let per_block = self.block_size / 32;
        let block = self.gdt_block + group / per_block;
        let off = (group % per_block) as usize * 32;
        let buf = self.read_block(block)?;
        read_struct(&buf, off).ok_or(EIO)
    }

    /// Reads inode number `ino` (`ext4_iget`, on-disk part).
    pub fn read_inode(&self, ino: u32) -> Result<Ext2Inode> {
        if ino == 0 {
            return Err(EINVAL);
        }
        let group = (ino - 1) / self.inodes_per_group;
        let index = (ino - 1) % self.inodes_per_group;
        let gd = self.read_group_desc(group)?;
        let byte = index as u64 * self.inode_size as u64;
        let block = gd.bg_inode_table as u64 + byte / self.block_size as u64;
        let off = (byte % self.block_size as u64) as usize;
        let buf = self.read_block(block as u32)?;
        read_struct(&buf, off).ok_or(EIO)
    }

    /// Maps file block `logical` to a disk block (`ext4_map_blocks`): the
    /// extent tree when `EXT4_EXTENTS_FL` is set, otherwise the ext2-style
    /// direct/single-indirect map.
    fn bmap(&self, inode: &Ext2Inode, logical: u32) -> Result<u32> {
        if inode.i_flags & EXT4_EXTENTS_FL != 0 {
            return self.extent_bmap(inode, logical);
        }
        // Classic indirect map (as in ext2).
        let ptrs = self.block_size as usize / 4;
        let l = logical as usize;
        if l < EXT2_NDIR_BLOCKS {
            return Ok(inode.i_block[l]);
        }
        let l = l - EXT2_NDIR_BLOCKS;
        if l < ptrs {
            let ind = inode.i_block[EXT2_IND_BLOCK];
            if ind == 0 {
                return Ok(0);
            }
            let buf = self.read_block(ind)?;
            return read_struct::<u32>(&buf, l * 4).ok_or(EIO);
        }
        Err(EINVAL)
    }

    /// Walks the extent tree to map `logical` (`ext4_ext_map_blocks`).
    fn extent_bmap(&self, inode: &Ext2Inode, logical: u32) -> Result<u32> {
        // The root node lives in the 60-byte i_block area of the inode.
        let mut node: Vec<u8> = Vec::with_capacity(60);
        for w in inode.i_block {
            node.extend_from_slice(&w.to_le_bytes());
        }
        loop {
            let eh: Ext4ExtentHeader = read_struct(&node, 0).ok_or(EIO)?;
            if eh.eh_magic != EXT4_EXT_MAGIC {
                return Err(EINVAL);
            }
            if eh.eh_depth == 0 {
                for i in 0..eh.eh_entries as usize {
                    let e: Ext4Extent = read_struct(&node, 12 + i * 12).ok_or(EIO)?;
                    let start = e.ee_block;
                    let end = start as u64 + e.len() as u64;
                    if logical as u64 >= start as u64 && (logical as u64) < end {
                        return Ok((e.start() + (logical - start) as u64) as u32);
                    }
                }
                return Ok(0); // a hole
            }
            // Index node: descend to the last child whose key is <= logical.
            let mut child = 0u64;
            let mut found = false;
            for i in 0..eh.eh_entries as usize {
                let idx: Ext4ExtentIdx = read_struct(&node, 12 + i * 12).ok_or(EIO)?;
                if idx.ei_block <= logical {
                    child = idx.leaf();
                    found = true;
                } else {
                    break;
                }
            }
            if !found {
                return Ok(0);
            }
            node = self.read_block(child as u32)?;
        }
    }

    /// Reads the whole contents of a regular file inode.
    pub fn read_file(&self, inode: &Ext2Inode) -> Result<Vec<u8>> {
        let size = inode.size() as usize;
        let bs = self.block_size as usize;
        let mut out = Vec::with_capacity(size);
        let nblocks = size.div_ceil(bs);
        for i in 0..nblocks {
            let take = (size - out.len()).min(bs);
            let phys = self.bmap(inode, i as u32)?;
            if phys == 0 {
                out.resize(out.len() + take, 0);
            } else {
                let buf = self.read_block(phys)?;
                out.extend_from_slice(&buf[..take]);
            }
        }
        Ok(out)
    }

    /// Calls `f(name, inode, file_type)` for each entry of directory `dir`.
    pub fn read_dir(&self, dir: &Ext2Inode, mut f: impl FnMut(&[u8], u32, u8)) -> Result {
        let bs = self.block_size as usize;
        let nblocks = (dir.size() as usize).div_ceil(bs);
        for i in 0..nblocks {
            let phys = self.bmap(dir, i as u32)?;
            if phys == 0 {
                continue;
            }
            let buf = self.read_block(phys)?;
            let mut off = 0usize;
            while off + 8 <= bs {
                let inode = read_struct::<u32>(&buf, off).ok_or(EIO)?;
                let rec_len = read_struct::<u16>(&buf, off + 4).ok_or(EIO)? as usize;
                let name_len = buf[off + 6] as usize;
                let file_type = buf[off + 7];
                if rec_len < 8 || off + rec_len > bs {
                    break;
                }
                if inode != 0 && off + 8 + name_len <= bs {
                    f(&buf[off + 8..off + 8 + name_len], inode, file_type);
                }
                off += rec_len;
            }
        }
        Ok(())
    }

    /// Finds `name` in directory `dir` (`ext4_lookup`).
    pub fn lookup(&self, dir: &Ext2Inode, name: &[u8]) -> Result<u32> {
        let mut found = 0u32;
        self.read_dir(dir, |n, ino, _| {
            if found == 0 && n == name {
                found = ino;
            }
        })?;
        if found == 0 { Err(ENOENT) } else { Ok(found) }
    }

    /// Resolves an absolute path to its inode.
    pub fn namei(&self, path: &[u8]) -> Result<(u32, Ext2Inode)> {
        let mut ino = EXT2_ROOT_INO;
        let mut inode = self.read_inode(ino)?;
        for comp in path.split(|&c| c == b'/') {
            if comp.is_empty() {
                continue;
            }
            if !inode.is_dir() {
                return Err(ENOENT);
            }
            ino = self.lookup(&inode, comp)?;
            inode = self.read_inode(ino)?;
        }
        Ok((ino, inode))
    }

    /// Reads a file by absolute path.
    pub fn read_path(&self, path: &[u8]) -> Result<Vec<u8>> {
        let (_, inode) = self.namei(path)?;
        if !inode.is_reg() {
            return Err(EINVAL);
        }
        self.read_file(&inode)
    }
}

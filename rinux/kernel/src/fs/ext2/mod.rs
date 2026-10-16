// SPDX-License-Identifier: GPL-2.0
//! A read-only ext2 filesystem.
//!
//! A small port of the read paths of `fs/ext2/super.c` (mount: read and
//! validate the superblock), `fs/ext2/inode.c` (locate an inode, map a file
//! block to a disk block through the direct and single-indirect pointers)
//! and `fs/ext2/dir.c` (walk directory blocks, resolve a name). It mounts an
//! ext2 image on a [`BlockDevice`] and reads files and directories.
//!
//! Simplified from Linux's: read-only, no page cache (every access hits the
//! block device), no inode cache, only single-indirect blocks (so files up
//! to `12 + block_size/4` blocks), and none of the ext3/ext4 features, ACLs,
//! xattrs or the buddy block allocator.

pub mod layout;

use alloc::boxed::Box;
use alloc::vec::Vec;

use rinux_lib::errno::{EINVAL, EIO, ENOENT, Result};

use crate::block::BlockDevice;
use layout::*;

/// A mounted ext2 filesystem.
pub struct Ext2Fs {
    dev: Box<dyn BlockDevice>,
    block_size: u32,
    first_data_block: u32,
    inodes_per_group: u32,
    inode_size: u32,
    /// First block of the group descriptor table.
    gdt_block: u32,
}

impl Ext2Fs {
    /// `ext2_fill_super()`: read the superblock, validate it, and record the
    /// geometry needed to find inodes and blocks.
    pub fn mount(dev: Box<dyn BlockDevice>) -> Result<Ext2Fs> {
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
        Ok(Ext2Fs {
            dev,
            block_size,
            first_data_block: sb.s_first_data_block,
            inodes_per_group: sb.s_inodes_per_group,
            inode_size,
            // The group descriptor table follows the superblock's block.
            gdt_block: sb.s_first_data_block + 1,
        })
    }

    /// Reads filesystem block `block` into a fresh buffer.
    fn read_block(&self, block: u32) -> Result<Vec<u8>> {
        let bs = self.block_size as usize;
        let mut buf = alloc::vec![0u8; bs];
        let sector = block as u64 * (bs as u64 / 512);
        self.dev.read(sector, &mut buf)?;
        Ok(buf)
    }

    /// `ext2_get_group_desc()`: the descriptor for block group `group`.
    fn read_group_desc(&self, group: u32) -> Result<Ext2GroupDesc> {
        let per_block = self.block_size / 32;
        let block = self.gdt_block + group / per_block;
        let off = (group % per_block) as usize * 32;
        let buf = self.read_block(block)?;
        read_struct(&buf, off).ok_or(EIO)
    }

    /// `ext2_iget()` (on-disk part): read inode number `ino`.
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

    /// `ext2_get_block()`: map file block `logical` of `inode` to a disk
    /// block (0 means a hole). Handles the direct and single-indirect
    /// pointers.
    fn bmap(&self, inode: &Ext2Inode, logical: u32) -> Result<u32> {
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
        // Double/triple indirect blocks are not supported.
        Err(EINVAL)
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
                // A hole reads as zeros.
                out.resize(out.len() + take, 0);
            } else {
                let buf = self.read_block(phys)?;
                out.extend_from_slice(&buf[..take]);
            }
        }
        Ok(out)
    }

    /// `ext2_readdir()`: call `f(name, inode, file_type)` for each live entry
    /// of directory `dir`.
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

    /// `ext2_lookup()`: find `name` in directory `dir`, returning its inode
    /// number.
    pub fn lookup(&self, dir: &Ext2Inode, name: &[u8]) -> Result<u32> {
        let mut found = 0u32;
        self.read_dir(dir, |n, ino, _| {
            if found == 0 && n == name {
                found = ino;
            }
        })?;
        if found == 0 { Err(ENOENT) } else { Ok(found) }
    }

    /// Resolves an absolute path to its inode (`path_lookup` for ext2).
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

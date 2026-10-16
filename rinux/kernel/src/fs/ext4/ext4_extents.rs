// SPDX-License-Identifier: GPL-2.0
//! The ext4 extent tree on-disk format.
//!
//! A subset of `fs/ext4/ext4_extents.h`: the extent-tree header, leaf extent
//! and index entries, plus the magic and the inode flag that selects extents
//! over the classic indirect block map.

/// `eh_magic` for a valid extent-tree node.
pub const EXT4_EXT_MAGIC: u16 = 0xf30a;
/// `i_flags` bit: the inode's `i_block` holds an extent tree, not block
/// pointers.
pub const EXT4_EXTENTS_FL: u32 = 0x0008_0000;

/// `struct ext4_extent_header`: starts every extent-tree node. 12 bytes.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Ext4ExtentHeader {
    pub eh_magic: u16,
    pub eh_entries: u16,
    pub eh_max: u16,
    /// 0 for a leaf (entries are [`Ext4Extent`]); >0 for an index node.
    pub eh_depth: u16,
    pub eh_generation: u32,
}

/// `struct ext4_extent`: a leaf, mapping a run of logical blocks. 12 bytes.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Ext4Extent {
    /// First logical block this extent covers.
    pub ee_block: u32,
    /// Number of blocks covered (>32768 marks an uninitialized extent).
    pub ee_len: u16,
    pub ee_start_hi: u16,
    pub ee_start_lo: u32,
}

impl Ext4Extent {
    /// The starting physical block (48-bit).
    pub fn start(&self) -> u64 {
        ((self.ee_start_hi as u64) << 32) | self.ee_start_lo as u64
    }
    /// The number of initialized blocks covered.
    pub fn len(&self) -> u32 {
        if self.ee_len > 32768 {
            (self.ee_len - 32768) as u32
        } else {
            self.ee_len as u32
        }
    }
}

/// `struct ext4_extent_idx`: an internal node, pointing at a child. 12 bytes.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Ext4ExtentIdx {
    /// This child covers logical blocks from here on.
    pub ei_block: u32,
    pub ei_leaf_lo: u32,
    pub ei_leaf_hi: u16,
    pub ei_unused: u16,
}

impl Ext4ExtentIdx {
    /// The physical block of the child node (48-bit).
    pub fn leaf(&self) -> u64 {
        ((self.ei_leaf_hi as u64) << 32) | self.ei_leaf_lo as u64
    }
}

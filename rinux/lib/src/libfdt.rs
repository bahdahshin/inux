// SPDX-License-Identifier: GPL-2.0-or-later OR BSD-2-Clause
//! Read-only access to a flattened device tree, ported from libfdt.
//!
//! Ported from `scripts/dtc/libfdt/fdt.c`, `fdt_ro.c` and `fdt_strerror.c`:
//! the header checks, the structure-block walk, property, string and path
//! lookups, phandles, memory reservations and the string-list helpers. The
//! blob is a byte slice. Every field is read big-endian with a bounds check,
//! so no pointer alignment is needed. Offsets are byte offsets into the blob,
//! as in C. A C error code is an [`FdtError`]; a C "offset or negative error"
//! is a `Result<usize, FdtError>`.
//!
//! Deviations from C: the `can_assume()` fast paths are not ported, so every
//! check runs; the pointer alignment check is dropped because reads are
//! bytewise; a blob whose `totalsize` runs past its slice is
//! [`FdtError::Truncated`]; and a blob shorter than a version 17 header
//! (40 bytes) is rejected up front. Functions that return a C `const char *`
//! return the bytes without the terminating NUL. Functions that return a
//! `char *` value and its length return the slice, whose length is the length.

const FDT_MAGIC: u32 = 0xd00d_feed;
const FDT_SW_MAGIC: u32 = !FDT_MAGIC;
const FDT_TAGSIZE: usize = 4;
/// `FDT_MAX_PHANDLE`: the largest phandle the tree may hold.
pub const FDT_MAX_PHANDLE: u32 = 0xffff_fffe;
const FDT_FIRST_SUPPORTED_VERSION: u32 = 0x02;
const FDT_LAST_SUPPORTED_VERSION: u32 = 0x11;

const FDT_BEGIN_NODE: u32 = 0x1;
const FDT_END_NODE: u32 = 0x2;
const FDT_PROP: u32 = 0x3;
const FDT_NOP: u32 = 0x4;
const FDT_END: u32 = 0x9;

/// `struct fdt_property` with its tag: tag, len, nameoff, then the value.
const PROP_HEADER: usize = 12;
/// `struct fdt_reserve_entry`: address and size, both big-endian u64.
const RSV_ENTRY: usize = 16;

const FDT_V1_SIZE: usize = 7 * 4;
const FDT_V2_SIZE: usize = 8 * 4;
const FDT_V3_SIZE: usize = 9 * 4;
const FDT_V16_SIZE: usize = FDT_V3_SIZE;
const FDT_V17_SIZE: usize = 10 * 4;

// Byte offsets of the header fields.
const H_MAGIC: usize = 0;
const H_TOTALSIZE: usize = 4;
const H_OFF_DT_STRUCT: usize = 8;
const H_OFF_DT_STRINGS: usize = 12;
const H_OFF_MEM_RSVMAP: usize = 16;
const H_VERSION: usize = 20;
const H_LAST_COMP: usize = 24;
const H_SIZE_DT_STRINGS: usize = 32;
const H_SIZE_DT_STRUCT: usize = 36;

/// The libfdt error codes. The C macros are negative; these hold the
/// positive value, and [`strerror`] takes the negative one as C does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum FdtError {
    /// `FDT_ERR_NOTFOUND`: the requested node or property does not exist.
    NotFound = 1,
    /// `FDT_ERR_EXISTS`: a node or property to create already exists.
    Exists = 2,
    /// `FDT_ERR_NOSPACE`: the operation needs more room than the buffer has.
    NoSpace = 3,
    /// `FDT_ERR_BADOFFSET`: the offset is not a node or property of the right kind.
    BadOffset = 4,
    /// `FDT_ERR_BADPATH`: a path is malformed or names nothing.
    BadPath = 5,
    /// `FDT_ERR_BADPHANDLE`: the phandle is 0 or all ones.
    BadPhandle = 6,
    /// `FDT_ERR_BADSTATE`: the blob is an unfinished sequential-write tree.
    BadState = 7,
    /// `FDT_ERR_TRUNCATED`: the blob or a block is shorter than it claims.
    Truncated = 8,
    /// `FDT_ERR_BADMAGIC`: the blob does not start with a device tree magic.
    BadMagic = 9,
    /// `FDT_ERR_BADVERSION`: the version is outside the supported range.
    BadVersion = 10,
    /// `FDT_ERR_BADSTRUCTURE`: the structure block is corrupt.
    BadStructure = 11,
    /// `FDT_ERR_BADLAYOUT`: a read-write operation hit an unsupported layout.
    BadLayout = 12,
    /// `FDT_ERR_INTERNAL`: an internal consistency check failed.
    Internal = 13,
    /// `FDT_ERR_BADNCELLS`: `#address-cells` or `#size-cells` is out of range.
    BadNcells = 14,
    /// `FDT_ERR_BADVALUE`: a property has a value of the wrong shape.
    BadValue = 15,
    /// `FDT_ERR_BADOVERLAY`: an overlay cannot be applied.
    BadOverlay = 16,
    /// `FDT_ERR_NOPHANDLES`: every phandle value is in use.
    NoPhandles = 17,
    /// `FDT_ERR_BADFLAGS`: a flags argument has bits the function does not take.
    BadFlags = 18,
    /// `FDT_ERR_ALIGNMENT`: a block sits at an offset it must not.
    Alignment = 19,
}

/// Every error, in code order, so the table index is the code minus one.
const ERRORS: [FdtError; 19] = [
    FdtError::NotFound,
    FdtError::Exists,
    FdtError::NoSpace,
    FdtError::BadOffset,
    FdtError::BadPath,
    FdtError::BadPhandle,
    FdtError::BadState,
    FdtError::Truncated,
    FdtError::BadMagic,
    FdtError::BadVersion,
    FdtError::BadStructure,
    FdtError::BadLayout,
    FdtError::Internal,
    FdtError::BadNcells,
    FdtError::BadValue,
    FdtError::BadOverlay,
    FdtError::NoPhandles,
    FdtError::BadFlags,
    FdtError::Alignment,
];

impl FdtError {
    /// The positive code of the error; the C macro is this value negated.
    pub fn code(self) -> i32 {
        self as i32
    }

    /// The C macro name, as `fdt_strerror()` prints it.
    pub fn name(self) -> &'static str {
        match self {
            FdtError::NotFound => "FDT_ERR_NOTFOUND",
            FdtError::Exists => "FDT_ERR_EXISTS",
            FdtError::NoSpace => "FDT_ERR_NOSPACE",
            FdtError::BadOffset => "FDT_ERR_BADOFFSET",
            FdtError::BadPath => "FDT_ERR_BADPATH",
            FdtError::BadPhandle => "FDT_ERR_BADPHANDLE",
            FdtError::BadState => "FDT_ERR_BADSTATE",
            FdtError::Truncated => "FDT_ERR_TRUNCATED",
            FdtError::BadMagic => "FDT_ERR_BADMAGIC",
            FdtError::BadVersion => "FDT_ERR_BADVERSION",
            FdtError::BadStructure => "FDT_ERR_BADSTRUCTURE",
            FdtError::BadLayout => "FDT_ERR_BADLAYOUT",
            FdtError::Internal => "FDT_ERR_INTERNAL",
            FdtError::BadNcells => "FDT_ERR_BADNCELLS",
            FdtError::BadValue => "FDT_ERR_BADVALUE",
            FdtError::BadOverlay => "FDT_ERR_BADOVERLAY",
            FdtError::NoPhandles => "FDT_ERR_NOPHANDLES",
            FdtError::BadFlags => "FDT_ERR_BADFLAGS",
            FdtError::Alignment => "FDT_ERR_ALIGNMENT",
        }
    }

    /// The error for a C-style negative return value, if it has one.
    fn from_errval(errval: i32) -> Option<Self> {
        ERRORS
            .iter()
            .copied()
            .find(|e| errval.checked_neg() == Some(e.code()))
    }
}

/// `fdt_strerror()`: the name of a C-style return value. A positive value is
/// an offset or length, zero is success, and an unknown code is named as such.
pub fn strerror(errval: i32) -> &'static str {
    if errval > 0 {
        "<valid offset/length>"
    } else if errval == 0 {
        "<no error>"
    } else {
        FdtError::from_errval(errval).map_or("<unknown error>", FdtError::name)
    }
}

/// The header size for a version, as `fdt_header_size_()`.
fn header_size_for(version: u32) -> usize {
    if version <= 1 {
        FDT_V1_SIZE
    } else if version <= 2 {
        FDT_V2_SIZE
    } else if version <= 3 {
        FDT_V3_SIZE
    } else if version <= 16 {
        FDT_V16_SIZE
    } else {
        FDT_V17_SIZE
    }
}

fn be32(buf: &[u8], off: usize) -> Option<u32> {
    let b = buf.get(off..off.checked_add(4)?)?;
    Some(u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
}

fn be64(buf: &[u8], off: usize) -> Option<u64> {
    let b = buf.get(off..off.checked_add(8)?)?;
    Some(u64::from_be_bytes([
        b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
    ]))
}

/// A header field. Callers have checked the length first, so the default is
/// never taken.
fn hdr(buf: &[u8], off: usize) -> u32 {
    be32(buf, off).unwrap_or(0)
}

/// `fdt_ro_probe_()`: the sanity checks that let a blob be read, returning
/// its `totalsize`. A sequential-write blob is accepted once it has a
/// structure block.
fn ro_probe(buf: &[u8]) -> Result<usize, FdtError> {
    if buf.len() < FDT_V17_SIZE {
        return Err(FdtError::Truncated);
    }
    match hdr(buf, H_MAGIC) {
        FDT_MAGIC => {
            if hdr(buf, H_VERSION) < FDT_FIRST_SUPPORTED_VERSION
                || hdr(buf, H_LAST_COMP) > FDT_LAST_SUPPORTED_VERSION
            {
                return Err(FdtError::BadVersion);
            }
        }
        FDT_SW_MAGIC => {
            if hdr(buf, H_SIZE_DT_STRUCT) == 0 {
                return Err(FdtError::BadState);
            }
        }
        _ => return Err(FdtError::BadMagic),
    }
    let totalsize = hdr(buf, H_TOTALSIZE) as usize;
    if totalsize >= i32::MAX as usize {
        return Err(FdtError::Truncated);
    }
    Ok(totalsize)
}

/// `check_off_()`: an offset must lie between the header and the end.
fn check_off(hdrsize: usize, totalsize: usize, off: usize) -> bool {
    off >= hdrsize && off <= totalsize
}

/// `check_block_()`: a block must start and end inside the body.
fn check_block(hdrsize: usize, totalsize: usize, base: usize, size: usize) -> bool {
    check_off(hdrsize, totalsize, base) && check_off(hdrsize, totalsize, base + size)
}

/// The token at a position in the structure block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Token {
    BeginNode,
    Prop,
    EndNode,
    Nop,
    /// The real end of the structure block.
    End,
}

/// A property as stored: its name offset and its value bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Property<'a> {
    /// Offset of the name in the strings block (`nameoff`).
    pub nameoff: u32,
    /// The value. Its length is the stored `len`.
    pub data: &'a [u8],
}

/// A read-only device tree blob, cut to its `totalsize`.
#[derive(Debug, Clone, Copy)]
pub struct Fdt<'a> {
    buf: &'a [u8],
}

impl<'a> Fdt<'a> {
    /// `fdt_ro_probe_()` plus the cut to `totalsize`: the entry point for every
    /// read. The blob may be a finished tree or a sequential-write one.
    pub fn new(buf: &'a [u8]) -> Result<Self, FdtError> {
        let totalsize = ro_probe(buf)?;
        if totalsize < FDT_V17_SIZE {
            return Err(FdtError::Truncated);
        }
        let blob = buf.get(..totalsize).ok_or(FdtError::Truncated)?;
        Ok(Self { buf: blob })
    }

    /// `fdt_check_header()`: the full structural check of a finished blob. It
    /// reads the raw slice, so it runs before [`Fdt::new`] is trusted.
    pub fn check_header(buf: &[u8]) -> Result<(), FdtError> {
        if buf.len() < FDT_V17_SIZE {
            return Err(FdtError::Truncated);
        }
        if hdr(buf, H_MAGIC) != FDT_MAGIC {
            return Err(FdtError::BadMagic);
        }
        let version = hdr(buf, H_VERSION);
        let last_comp = hdr(buf, H_LAST_COMP);
        if version < FDT_FIRST_SUPPORTED_VERSION
            || last_comp > FDT_LAST_SUPPORTED_VERSION
            || version < last_comp
        {
            return Err(FdtError::BadVersion);
        }
        let hdrsize = header_size_for(version);
        let totalsize = hdr(buf, H_TOTALSIZE) as usize;
        if totalsize < hdrsize || totalsize > i32::MAX as usize || totalsize > buf.len() {
            return Err(FdtError::Truncated);
        }
        let off_mem = hdr(buf, H_OFF_MEM_RSVMAP) as usize;
        let off_struct = hdr(buf, H_OFF_DT_STRUCT) as usize;
        let off_strings = hdr(buf, H_OFF_DT_STRINGS) as usize;
        if off_mem % 8 != 0 {
            return Err(FdtError::Alignment);
        }
        if off_struct % FDT_TAGSIZE != 0 {
            return Err(FdtError::Alignment);
        }
        if !check_off(hdrsize, totalsize, off_mem) {
            return Err(FdtError::Truncated);
        }
        let size_struct = hdr(buf, H_SIZE_DT_STRUCT) as usize;
        if version < 17 {
            if !check_off(hdrsize, totalsize, off_struct) {
                return Err(FdtError::Truncated);
            }
        } else if !check_block(hdrsize, totalsize, off_struct, size_struct) {
            return Err(FdtError::Truncated);
        }
        let size_strings = hdr(buf, H_SIZE_DT_STRINGS) as usize;
        if !check_block(hdrsize, totalsize, off_strings, size_strings) {
            return Err(FdtError::Truncated);
        }
        Ok(())
    }

    /// `fdt_magic()`.
    pub fn magic(&self) -> u32 {
        hdr(self.buf, H_MAGIC)
    }

    /// `fdt_version()`.
    pub fn version(&self) -> u32 {
        hdr(self.buf, H_VERSION)
    }

    /// `fdt_last_comp_version()`.
    pub fn last_comp_version(&self) -> u32 {
        hdr(self.buf, H_LAST_COMP)
    }

    /// `fdt_totalsize()`.
    pub fn totalsize(&self) -> usize {
        self.buf.len()
    }

    /// `fdt_off_dt_struct()`.
    pub fn off_dt_struct(&self) -> usize {
        hdr(self.buf, H_OFF_DT_STRUCT) as usize
    }

    /// `fdt_off_dt_strings()`.
    pub fn off_dt_strings(&self) -> usize {
        hdr(self.buf, H_OFF_DT_STRINGS) as usize
    }

    /// `fdt_off_mem_rsvmap()`.
    pub fn off_mem_rsvmap(&self) -> usize {
        hdr(self.buf, H_OFF_MEM_RSVMAP) as usize
    }

    /// `fdt_size_dt_strings()`.
    pub fn size_dt_strings(&self) -> usize {
        hdr(self.buf, H_SIZE_DT_STRINGS) as usize
    }

    /// `fdt_size_dt_struct()`.
    pub fn size_dt_struct(&self) -> usize {
        hdr(self.buf, H_SIZE_DT_STRUCT) as usize
    }

    /// `fdt_header_size()`: the header size this blob's version implies.
    pub fn header_size(&self) -> usize {
        header_size_for(self.version())
    }

    /// `fdt_offset_ptr()`: `len` bytes of the structure block at `offset`, if
    /// they lie inside the blob and, for version 17 and up, inside the block.
    pub fn offset_ptr(&self, offset: usize, len: usize) -> Option<&'a [u8]> {
        let buf = self.buf;
        let abs = self.off_dt_struct().checked_add(offset)?;
        let end = abs.checked_add(len)?;
        if end > buf.len() {
            return None;
        }
        if self.version() >= 0x11 && offset.checked_add(len)? > self.size_dt_struct() {
            return None;
        }
        buf.get(abs..end)
    }

    /// `fdt_next_tag()`. Returns the token at `offset` and the offset of the
    /// next token. A premature end or a bad token is an `Err`, which the C
    /// code reports as `FDT_END` with a negative next offset.
    fn next_tag(&self, offset: usize) -> Result<(Token, usize), FdtError> {
        let tag_bytes = self.offset_ptr(offset, FDT_TAGSIZE).ok_or(FdtError::Truncated)?;
        let tag = be32(tag_bytes, 0).ok_or(FdtError::Truncated)?;
        let mut off = offset + FDT_TAGSIZE;

        let token = match tag {
            FDT_BEGIN_NODE => {
                // Skip the name and its NUL.
                loop {
                    let b = self.offset_ptr(off, 1).ok_or(FdtError::BadStructure)?[0];
                    off += 1;
                    if b == 0 {
                        break;
                    }
                }
                Token::BeginNode
            }
            FDT_PROP => {
                let len_bytes = self.offset_ptr(off, 4).ok_or(FdtError::BadStructure)?;
                let len = be32(len_bytes, 0).ok_or(FdtError::BadStructure)?;
                let off32 = off as u32;
                let sum = len.wrapping_add(off32);
                if sum >= i32::MAX as u32 || sum < off32 {
                    return Err(FdtError::BadStructure);
                }
                // Skip the len and nameoff fields, then the value.
                off += PROP_HEADER - FDT_TAGSIZE + len as usize;
                if self.version() < 0x10
                    && len >= 8
                    && (off as u32).wrapping_sub(len) % 8 != 0
                {
                    off += 4;
                }
                Token::Prop
            }
            FDT_END_NODE => Token::EndNode,
            FDT_NOP => Token::Nop,
            FDT_END => Token::End,
            _ => return Err(FdtError::BadStructure),
        };

        if self.offset_ptr(offset, off - offset).is_none() {
            return Err(FdtError::BadStructure);
        }
        Ok((token, (off + 3) & !3))
    }

    /// `fdt_check_node_offset_()`: the offset of a node's BEGIN token, and the
    /// offset just past it.
    fn check_node_offset(&self, offset: usize) -> Result<usize, FdtError> {
        if offset % FDT_TAGSIZE != 0 {
            return Err(FdtError::BadOffset);
        }
        let (token, next) = self.next_tag(offset).map_err(|_| FdtError::BadOffset)?;
        if token != Token::BeginNode {
            return Err(FdtError::BadOffset);
        }
        Ok(next)
    }

    /// `fdt_check_prop_offset_()`: the offset of a property token.
    fn check_prop_offset(&self, offset: usize) -> Result<usize, FdtError> {
        if offset % FDT_TAGSIZE != 0 {
            return Err(FdtError::BadOffset);
        }
        let (token, next) = self.next_tag(offset).map_err(|_| FdtError::BadOffset)?;
        if token != Token::Prop {
            return Err(FdtError::BadOffset);
        }
        Ok(next)
    }

    /// `fdt_next_node()`. From `offset` (or the root when `None`) it finds the
    /// next node. `depth` is kept as the nesting level, with a node below the
    /// start at depth 1 and a return to a shallower level below zero.
    pub fn next_node(
        &self,
        offset: Option<usize>,
        mut depth: Option<&mut i32>,
    ) -> Result<usize, FdtError> {
        let mut next = match offset {
            Some(o) => self.check_node_offset(o)?,
            None => 0,
        };
        loop {
            let cur = next;
            let token = match self.next_tag(cur) {
                Ok((token, after)) => {
                    next = after;
                    token
                }
                // A premature end is only the end of the tree without depth.
                Err(FdtError::Truncated) if depth.is_none() => return Err(FdtError::NotFound),
                Err(e) => return Err(e),
            };
            match token {
                Token::Prop | Token::Nop => {}
                Token::BeginNode => {
                    if let Some(d) = depth.as_deref_mut() {
                        *d += 1;
                    }
                }
                Token::EndNode => {
                    if let Some(d) = depth.as_deref_mut() {
                        *d -= 1;
                        if *d < 0 {
                            return Ok(next);
                        }
                    }
                }
                Token::End => return Err(FdtError::NotFound),
            }
            if token == Token::BeginNode {
                return Ok(cur);
            }
        }
    }

    /// `fdt_first_subnode()`: the first child of `parent`.
    pub fn first_subnode(&self, parent: usize) -> Result<usize, FdtError> {
        let mut depth = 0;
        let offset = self
            .next_node(Some(parent), Some(&mut depth))
            .map_err(|_| FdtError::NotFound)?;
        if depth != 1 {
            return Err(FdtError::NotFound);
        }
        Ok(offset)
    }

    /// `fdt_next_subnode()`: the next sibling of `offset`.
    pub fn next_subnode(&self, offset: usize) -> Result<usize, FdtError> {
        let mut depth = 1;
        let mut cur = offset;
        loop {
            cur = self
                .next_node(Some(cur), Some(&mut depth))
                .map_err(|_| FdtError::NotFound)?;
            if depth < 1 {
                return Err(FdtError::NotFound);
            }
            if depth <= 1 {
                return Ok(cur);
            }
        }
    }

    /// `fdt_get_string()`: the NUL-terminated string at `stroffset` in the
    /// strings block, without its NUL. A negative offset counts back from the
    /// end of a sequential-write blob's strings.
    pub fn get_string(&self, stroffset: i32) -> Result<&'a [u8], FdtError> {
        let buf = self.buf;
        let totalsize = buf.len() as i64;
        let absoffset = self.off_dt_strings() as i64 + stroffset as i64;
        if absoffset < 0 || absoffset >= totalsize {
            return Err(FdtError::BadOffset);
        }
        let mut len = totalsize - absoffset;
        match self.magic() {
            FDT_MAGIC => {
                if stroffset < 0 {
                    return Err(FdtError::BadOffset);
                }
                if self.version() >= 17 {
                    let size = self.size_dt_strings() as i64;
                    if stroffset as i64 >= size {
                        return Err(FdtError::BadOffset);
                    }
                    if size - (stroffset as i64) < len {
                        len = size - stroffset as i64;
                    }
                }
            }
            FDT_SW_MAGIC => {
                let sw = -(stroffset as i64);
                if stroffset >= 0 || sw > self.size_dt_strings() as i64 {
                    return Err(FdtError::BadOffset);
                }
                if sw < len {
                    len = sw;
                }
            }
            _ => return Err(FdtError::Internal),
        }
        let start = absoffset as usize;
        let s = buf
            .get(start..start + len as usize)
            .ok_or(FdtError::Truncated)?;
        let n = s
            .iter()
            .position(|&c| c == 0)
            .ok_or(FdtError::Truncated)?;
        Ok(&s[..n])
    }

    /// `fdt_string_eq_()`: whether the string at `stroffset` is `s`.
    fn string_eq(&self, stroffset: u32, s: &[u8]) -> bool {
        self.get_string(stroffset as i32).is_ok_and(|p| p == s)
    }

    /// `fdt_find_max_phandle()`: the largest phandle in use, or 0.
    pub fn find_max_phandle(&self) -> Result<u32, FdtError> {
        let mut max = 0;
        let mut offset = None;
        loop {
            match self.next_node(offset, None) {
                Ok(o) => {
                    offset = Some(o);
                    max = max.max(self.get_phandle(o));
                }
                Err(FdtError::NotFound) => return Ok(max),
                Err(e) => return Err(e),
            }
        }
    }

    /// `fdt_generate_phandle()`: one more than the largest phandle in use.
    pub fn generate_phandle(&self) -> Result<u32, FdtError> {
        let max = self.find_max_phandle()?;
        if max == FDT_MAX_PHANDLE {
            return Err(FdtError::NoPhandles);
        }
        Ok(max + 1)
    }

    /// `fdt_mem_rsv()`: the `n`th reservation as (address, size), if present.
    fn mem_rsv_entry(&self, n: usize) -> Option<(u64, u64)> {
        let absoffset = self.off_mem_rsvmap().checked_add(n.checked_mul(RSV_ENTRY)?)?;
        let e = self.buf.get(absoffset..absoffset.checked_add(RSV_ENTRY)?)?;
        Some((be64(e, 0)?, be64(e, 8)?))
    }

    /// `fdt_get_mem_rsv()`: the `n`th memory reservation.
    pub fn get_mem_rsv(&self, n: usize) -> Result<(u64, u64), FdtError> {
        self.mem_rsv_entry(n).ok_or(FdtError::BadOffset)
    }

    /// `fdt_num_mem_rsv()`: the reservations before the zero-size terminator.
    pub fn num_mem_rsv(&self) -> Result<usize, FdtError> {
        let mut i = 0;
        while let Some((_, size)) = self.mem_rsv_entry(i) {
            if size == 0 {
                return Ok(i);
            }
            i += 1;
        }
        Err(FdtError::Truncated)
    }

    /// `nextprop_()`: the first property token at or after `offset`, skipping
    /// NOPs. A child node or the end of the node means there are none.
    fn next_prop(&self, mut offset: usize) -> Result<usize, FdtError> {
        loop {
            let (token, next) = self.next_tag(offset)?;
            match token {
                Token::End => return Err(FdtError::BadStructure),
                Token::Prop => return Ok(offset),
                Token::Nop => offset = next,
                _ => return Err(FdtError::NotFound),
            }
        }
    }

    /// `fdt_get_name()`: the name of a node. Before version 16 the name is
    /// given as its last path component, as version 16 names it.
    pub fn get_name(&self, node: usize) -> Result<&'a [u8], FdtError> {
        self.check_node_offset(node)?;
        let buf = self.buf;
        let start = self.off_dt_struct() + node + FDT_TAGSIZE;
        let rest = buf.get(start..).ok_or(FdtError::BadStructure)?;
        let n = rest
            .iter()
            .position(|&c| c == 0)
            .ok_or(FdtError::BadStructure)?;
        let mut name = &rest[..n];
        if self.version() < 0x10 {
            let i = name
                .iter()
                .rposition(|&c| c == b'/')
                .ok_or(FdtError::BadStructure)?;
            name = &name[i + 1..];
        }
        Ok(name)
    }

    /// `fdt_first_property_offset()`.
    pub fn first_property_offset(&self, node: usize) -> Result<usize, FdtError> {
        let offset = self.check_node_offset(node)?;
        self.next_prop(offset)
    }

    /// `fdt_next_property_offset()`.
    pub fn next_property_offset(&self, offset: usize) -> Result<usize, FdtError> {
        let offset = self.check_prop_offset(offset)?;
        self.next_prop(offset)
    }

    /// `fdt_get_property_by_offset_()`: the property at a property token.
    fn property_at(&self, offset: usize) -> Result<Property<'a>, FdtError> {
        self.check_prop_offset(offset)?;
        let hdr = self
            .offset_ptr(offset, PROP_HEADER)
            .ok_or(FdtError::BadStructure)?;
        let len = be32(hdr, 4).ok_or(FdtError::BadStructure)? as usize;
        let nameoff = be32(hdr, 8).ok_or(FdtError::BadStructure)?;
        let data = self
            .offset_ptr(offset + PROP_HEADER, len)
            .ok_or(FdtError::BadStructure)?;
        Ok(Property { nameoff, data })
    }

    /// `fdt_get_property_by_offset()`. Before version 16 a property may need
    /// realigning, and this does not handle that, so it is an error there.
    pub fn get_property_by_offset(&self, offset: usize) -> Result<Property<'a>, FdtError> {
        if self.version() < 0x10 {
            return Err(FdtError::BadVersion);
        }
        self.property_at(offset)
    }

    /// The value of a property, realigned for versions before 16 as C does.
    fn prop_value(&self, poffset: usize, prop: Property<'a>) -> Result<&'a [u8], FdtError> {
        if self.version() < 0x10
            && (poffset + PROP_HEADER) % 8 != 0
            && prop.data.len() >= 8
        {
            return self
                .offset_ptr(poffset + PROP_HEADER + 4, prop.data.len())
                .ok_or(FdtError::BadStructure);
        }
        Ok(prop.data)
    }

    /// `fdt_get_property_namelen_()`: the property named `name` on a node, with
    /// its token offset.
    fn find_property(
        &self,
        node: usize,
        name: &[u8],
    ) -> Result<(usize, Property<'a>), FdtError> {
        let mut offset = self.first_property_offset(node)?;
        loop {
            let prop = self.property_at(offset)?;
            if self.string_eq(prop.nameoff, name) {
                return Ok((offset, prop));
            }
            offset = self.next_property_offset(offset)?;
        }
    }

    /// `fdt_get_property_namelen()`. Before version 16 this is an error, as C.
    pub fn get_property(&self, node: usize, name: &[u8]) -> Result<Property<'a>, FdtError> {
        if self.version() < 0x10 {
            return Err(FdtError::BadVersion);
        }
        self.find_property(node, name).map(|(_, p)| p)
    }

    /// `fdt_getprop()`: the value of a property on a node, realigned for
    /// versions before 16.
    pub fn getprop(&self, node: usize, name: &[u8]) -> Result<&'a [u8], FdtError> {
        let (poffset, prop) = self.find_property(node, name)?;
        self.prop_value(poffset, prop)
    }

    /// `fdt_getprop_by_offset()`: the name and value of a property token.
    pub fn getprop_by_offset(&self, offset: usize) -> Result<(&'a [u8], &'a [u8]), FdtError> {
        let prop = self.property_at(offset)?;
        let name = self.get_string(prop.nameoff as i32)?;
        let value = self.prop_value(offset, prop)?;
        Ok((name, value))
    }

    /// `fdt_subnode_offset_namelen()`: the child of `parent` called `name`.
    /// A name without `@` also matches a child whose unit address follows.
    pub fn subnode_offset(&self, parent: usize, name: &[u8]) -> Result<usize, FdtError> {
        let mut depth = 0;
        let mut offset = parent;
        loop {
            if depth < 0 {
                return Err(FdtError::NotFound);
            }
            if depth == 1 && self.nodename_eq(offset, name) {
                return Ok(offset);
            }
            offset = self.next_node(Some(offset), Some(&mut depth))?;
        }
    }

    /// `fdt_nodename_eq_()`.
    fn nodename_eq(&self, offset: usize, s: &[u8]) -> bool {
        let p = match self.get_name(offset) {
            Ok(p) => p,
            Err(_) => return false,
        };
        if p.len() < s.len() || p[..s.len()] != *s {
            return false;
        }
        match p.get(s.len()) {
            None => true,
            Some(&b'@') => !s.contains(&b'@'),
            Some(_) => false,
        }
    }

    /// `fdt_path_offset()`: the node at a path, or at an alias that starts the
    /// path. The path must not be empty.
    pub fn path_offset(&self, path: &[u8]) -> Result<usize, FdtError> {
        let end = path.len();
        if end == 0 {
            return Err(FdtError::BadPath);
        }
        let mut p = 0;
        let mut offset = 0;
        if path[0] != b'/' {
            let q = path.iter().position(|&c| c == b'/').unwrap_or(end);
            let alias = self.get_alias(&path[..q]).map_err(|_| FdtError::BadPath)?;
            offset = self.path_offset(alias)?;
            p = q;
        }
        while p < end {
            while path[p] == b'/' {
                p += 1;
                if p == end {
                    return Ok(offset);
                }
            }
            let q = path[p..].iter().position(|&c| c == b'/').map_or(end, |i| p + i);
            offset = self.subnode_offset(offset, &path[p..q])?;
            p = q;
        }
        Ok(offset)
    }

    /// `fdt_get_phandle()`: the phandle of a node, from `phandle` or the older
    /// `linux,phandle`, or 0 when there is none.
    pub fn get_phandle(&self, node: usize) -> u32 {
        for name in [&b"phandle"[..], &b"linux,phandle"[..]] {
            if let Ok(v) = self.getprop(node, name) {
                if v.len() == 4 {
                    return be32(v, 0).unwrap_or(0);
                }
            }
        }
        0
    }

    /// `fdt_path_getprop_namelen()`: a property of the node at `path`.
    fn path_getprop(&self, path: &[u8], propname: &[u8]) -> Result<&'a [u8], FdtError> {
        let offset = self.path_offset(path)?;
        self.getprop(offset, propname)
    }

    /// `fdt_get_alias()`: the path an alias names, without its NUL.
    pub fn get_alias(&self, name: &[u8]) -> Result<&'a [u8], FdtError> {
        let alias = self.path_getprop(b"/aliases", name)?;
        if alias.last() == Some(&0) && alias.first() == Some(&b'/') {
            return Ok(&alias[..alias.len() - 1]);
        }
        Err(FdtError::NotFound)
    }

    /// `fdt_get_symbol()`: the path a `__symbols__` entry names, without its NUL.
    pub fn get_symbol(&self, name: &[u8]) -> Result<&'a [u8], FdtError> {
        let v = self.path_getprop(b"/__symbols__", name)?;
        Ok(v.strip_suffix(&[0u8]).unwrap_or(v))
    }

    /// `fdt_get_path()`: write the path of `nodeoffset` into `buf` with a NUL,
    /// and return its length. The root path is `/`.
    pub fn get_path(&self, nodeoffset: usize, buf: &mut [u8]) -> Result<usize, FdtError> {
        if buf.len() < 2 {
            return Err(FdtError::NoSpace);
        }
        let mut offset = 0;
        let mut depth = 0;
        let mut pdepth = 0;
        let mut p: usize = 0;
        let mut err = None;
        while offset <= nodeoffset {
            while pdepth > depth {
                // Back up to the '/' that ends the parent's name.
                loop {
                    p = p.checked_sub(1).ok_or(FdtError::Internal)?;
                    let q = p.checked_sub(1).ok_or(FdtError::Internal)?;
                    if buf.get(q) == Some(&b'/') {
                        break;
                    }
                }
                pdepth -= 1;
            }
            if pdepth >= depth {
                let name = self.get_name(offset)?;
                if p + name.len() < buf.len() {
                    buf[p..p + name.len()].copy_from_slice(name);
                    p += name.len();
                    *buf.get_mut(p).ok_or(FdtError::NoSpace)? = b'/';
                    p += 1;
                    pdepth += 1;
                }
            }
            if offset == nodeoffset {
                if pdepth < depth + 1 {
                    return Err(FdtError::NoSpace);
                }
                if p > 1 {
                    p -= 1;
                }
                *buf.get_mut(p).ok_or(FdtError::NoSpace)? = 0;
                return Ok(p);
            }
            match self.next_node(Some(offset), Some(&mut depth)) {
                Ok(n) => offset = n,
                Err(e) => {
                    err = Some(e);
                    break;
                }
            }
        }
        Err(match err {
            None | Some(FdtError::NotFound) => FdtError::BadOffset,
            Some(FdtError::BadOffset) => FdtError::BadStructure,
            Some(e) => e,
        })
    }

    /// `fdt_supernode_atdepth_offset()`: the ancestor of `nodeoffset` at
    /// `supernodedepth`, and the depth of `nodeoffset` itself.
    pub fn supernode_atdepth(
        &self,
        nodeoffset: usize,
        supernodedepth: i32,
    ) -> Result<(usize, i32), FdtError> {
        if supernodedepth < 0 {
            return Err(FdtError::NotFound);
        }
        let mut supernode = None;
        let mut offset = 0;
        let mut depth = 0;
        let mut err = None;
        while offset <= nodeoffset {
            if depth == supernodedepth {
                supernode = Some(offset);
            }
            if offset == nodeoffset {
                if supernodedepth > depth {
                    return Err(FdtError::NotFound);
                }
                return supernode.map(|s| (s, depth)).ok_or(FdtError::Internal);
            }
            match self.next_node(Some(offset), Some(&mut depth)) {
                Ok(n) => offset = n,
                Err(e) => {
                    err = Some(e);
                    break;
                }
            }
        }
        Err(match err {
            None | Some(FdtError::NotFound) => FdtError::BadOffset,
            Some(FdtError::BadOffset) => FdtError::BadStructure,
            Some(e) => e,
        })
    }

    /// `fdt_node_depth()`: how deep a node is, with the root at 0.
    pub fn node_depth(&self, nodeoffset: usize) -> Result<i32, FdtError> {
        self.supernode_atdepth(nodeoffset, 0).map(|(_, d)| d)
    }

    /// `fdt_parent_offset()`.
    pub fn parent_offset(&self, nodeoffset: usize) -> Result<usize, FdtError> {
        let depth = self.node_depth(nodeoffset)?;
        self.supernode_atdepth(nodeoffset, depth - 1)
            .map(|(s, _)| s)
    }

    /// `fdt_node_offset_by_prop_value()`: the first node after `start` whose
    /// `propname` has exactly `propval` as its value.
    pub fn node_offset_by_prop_value(
        &self,
        start: Option<usize>,
        propname: &[u8],
        propval: &[u8],
    ) -> Result<usize, FdtError> {
        let mut offset = self.next_node(start, None)?;
        loop {
            if let Ok(val) = self.getprop(offset, propname) {
                if val == propval {
                    return Ok(offset);
                }
            }
            offset = self.next_node(Some(offset), None)?;
        }
    }

    /// `fdt_node_offset_by_phandle()`.
    pub fn node_offset_by_phandle(&self, phandle: u32) -> Result<usize, FdtError> {
        if phandle == 0 || phandle == !0 {
            return Err(FdtError::BadPhandle);
        }
        let mut offset = self.next_node(None, None)?;
        loop {
            if self.get_phandle(offset) == phandle {
                return Ok(offset);
            }
            offset = self.next_node(Some(offset), None)?;
        }
    }

    /// `fdt_node_check_compatible()`: whether a node lists `compatible`.
    pub fn node_check_compatible(
        &self,
        nodeoffset: usize,
        compatible: &[u8],
    ) -> Result<bool, FdtError> {
        let prop = self.getprop(nodeoffset, b"compatible")?;
        Ok(stringlist_contains(prop, compatible))
    }

    /// `fdt_node_offset_by_compatible()`: the first node after `start` that
    /// lists `compatible`.
    pub fn node_offset_by_compatible(
        &self,
        start: Option<usize>,
        compatible: &[u8],
    ) -> Result<usize, FdtError> {
        let mut offset = self.next_node(start, None)?;
        loop {
            match self.node_check_compatible(offset, compatible) {
                Ok(true) => return Ok(offset),
                Ok(false) | Err(FdtError::NotFound) => {}
                Err(e) => return Err(e),
            }
            offset = self.next_node(Some(offset), None)?;
        }
    }

    /// `fdt_stringlist_count()`: the number of strings in a string-list property.
    pub fn stringlist_count(&self, node: usize, property: &[u8]) -> Result<usize, FdtError> {
        let mut rest = self.getprop(node, property)?;
        let mut count = 0;
        while !rest.is_empty() {
            let (_, next) = next_string(rest)?;
            rest = next;
            count += 1;
        }
        Ok(count)
    }

    /// `fdt_stringlist_search()`: the index of `string` in a string-list property.
    pub fn stringlist_search(
        &self,
        node: usize,
        property: &[u8],
        string: &[u8],
    ) -> Result<usize, FdtError> {
        let mut rest = self.getprop(node, property)?;
        let mut idx = 0;
        while !rest.is_empty() {
            let (s, next) = next_string(rest)?;
            if s == string {
                return Ok(idx);
            }
            rest = next;
            idx += 1;
        }
        Err(FdtError::NotFound)
    }

    /// `fdt_stringlist_get()`: the string at `idx` in a string-list property.
    pub fn stringlist_get(
        &self,
        node: usize,
        property: &[u8],
        idx: usize,
    ) -> Result<&'a [u8], FdtError> {
        let mut rest = self.getprop(node, property)?;
        let mut i = idx;
        while !rest.is_empty() {
            let (s, next) = next_string(rest)?;
            if i == 0 {
                return Ok(s);
            }
            rest = next;
            i -= 1;
        }
        Err(FdtError::NotFound)
    }

    /// `fdt_move()`: copy the whole blob into `buf`.
    pub fn move_to(&self, buf: &mut [u8]) -> Result<(), FdtError> {
        if buf.len() < self.buf.len() {
            return Err(FdtError::NoSpace);
        }
        buf[..self.buf.len()].copy_from_slice(self.buf);
        Ok(())
    }
}

/// The next NUL-terminated string in a string list, and the rest of the list.
/// A string with no NUL before the end is a `BadValue`.
fn next_string(list: &[u8]) -> Result<(&[u8], &[u8]), FdtError> {
    let n = list
        .iter()
        .position(|&c| c == 0)
        .ok_or(FdtError::BadValue)?;
    Ok((&list[..n], &list[n + 1..]))
}

/// `fdt_stringlist_contains()`: whether a string list holds `s`. Each string
/// in the list is matched whole, so `ab` does not match `abc`.
pub fn stringlist_contains(list: &[u8], s: &[u8]) -> bool {
    let mut rest = list;
    while rest.len() >= s.len() {
        if rest.starts_with(s) && rest.get(s.len()) == Some(&0) {
            return true;
        }
        match rest.iter().position(|&c| c == 0) {
            Some(p) => rest = &rest[p + 1..],
            None => return false,
        }
    }
    false
}

/// `fdt_find_string_len_()`: the offset in a strings table of `s` followed by
/// its NUL, if it is there.
pub fn find_string_len(strtab: &[u8], s: &[u8]) -> Option<usize> {
    let slen = s.len();
    if strtab.len() < slen + 1 {
        return None;
    }
    let last = strtab.len() - (slen + 1);
    (0..=last).find(|&p| strtab[p..p + slen] == *s && strtab[p + slen] == 0)
}

/// `FDT_LAST_COMPATIBLE_VERSION`: the version a finished sequential-write tree
/// is marked compatible with.
const FDT_LAST_COMPATIBLE_VERSION: usize = 0x10;
/// `FDT_MAX_NCELLS`: the largest `#address-cells` or `#size-cells`.
const FDT_MAX_NCELLS: u32 = 4;
/// `FDT_CREATE_FLAG_NO_NAME_DEDUP`: do not reuse an existing property name.
pub const FDT_CREATE_FLAG_NO_NAME_DEDUP: u32 = 0x1;
/// `FDT_ALIGN(sizeof(struct fdt_header), sizeof(struct fdt_reserve_entry))`:
/// where the first reservation sits in a new tree.
const HDR_SIZE_ALIGNED: usize = 48;
const H_BOOT_CPUID: usize = 28;

impl<'a> Fdt<'a> {
    /// `fdt_boot_cpuid_phys()`.
    pub fn boot_cpuid_phys(&self) -> u32 {
        hdr(self.buf, H_BOOT_CPUID)
    }

    /// `fdt_blocks_misordered_()`: whether the blocks are out of the order the
    /// writers expect (reservations, then structure, then strings, all inside).
    fn blocks_misordered(&self, mem_rsv_size: usize, struct_size: usize) -> bool {
        let off_mem = self.off_mem_rsvmap();
        let off_struct = self.off_dt_struct();
        let off_strings = self.off_dt_strings();
        off_mem < FDT_V17_SIZE
            || off_struct < off_mem + mem_rsv_size
            || off_strings < off_struct + struct_size
            || self.buf.len() < off_strings + self.size_dt_strings()
    }

    /// `fdt_node_end_offset_()`: the offset just past a node's END_NODE tag.
    fn node_end_offset(&self, node: usize) -> Result<usize, FdtError> {
        let mut depth = 0;
        let mut offset = node;
        loop {
            if depth < 0 {
                return Ok(offset);
            }
            offset = self.next_node(Some(offset), Some(&mut depth))?;
        }
    }

    /// `fdt_cells()`: a `#...-cells` property, which must be one 32-bit value
    /// no larger than [`FDT_MAX_NCELLS`].
    fn cells(&self, node: usize, name: &[u8]) -> Result<u32, FdtError> {
        let c = self.getprop(node, name)?;
        if c.len() != 4 {
            return Err(FdtError::BadNcells);
        }
        let val = be32(c, 0).ok_or(FdtError::BadNcells)?;
        if val > FDT_MAX_NCELLS {
            return Err(FdtError::BadNcells);
        }
        Ok(val)
    }

    /// `fdt_address_cells()`: the `#address-cells` of a node, defaulting to 2.
    pub fn address_cells(&self, node: usize) -> Result<u32, FdtError> {
        match self.cells(node, b"#address-cells") {
            Ok(0) => Err(FdtError::BadNcells),
            Ok(val) => Ok(val),
            Err(FdtError::NotFound) => Ok(2),
            Err(e) => Err(e),
        }
    }

    /// `fdt_size_cells()`: the `#size-cells` of a node, defaulting to 1.
    pub fn size_cells(&self, node: usize) -> Result<u32, FdtError> {
        match self.cells(node, b"#size-cells") {
            Ok(val) => Ok(val),
            Err(FdtError::NotFound) => Ok(1),
            Err(e) => Err(e),
        }
    }
}

/// Write `bytes` at `off`, failing if they do not fit the buffer.
fn put(buf: &mut [u8], off: usize, bytes: &[u8]) -> Result<(), FdtError> {
    buf.get_mut(off..off.checked_add(bytes.len()).ok_or(FdtError::Internal)?)
        .ok_or(FdtError::Internal)?
        .copy_from_slice(bytes);
    Ok(())
}

/// Zero `len` bytes at `off`.
fn zero(buf: &mut [u8], off: usize, len: usize) -> Result<(), FdtError> {
    buf.get_mut(off..off.checked_add(len).ok_or(FdtError::Internal)?)
        .ok_or(FdtError::Internal)?
        .fill(0);
    Ok(())
}

/// Zero the padding after `len` value bytes at `off`, up to a 4-byte boundary.
fn zero_pad(buf: &mut [u8], off: usize, len: usize) -> Result<(), FdtError> {
    zero(buf, off + len, align4(len) - len)
}

/// Set a header field (`fdt_set_*()`).
fn set_hdr(buf: &mut [u8], field: usize, v: usize) -> Result<(), FdtError> {
    put(buf, field, &(v as u32).to_be_bytes())
}

/// Move a header field by `delta` bytes.
fn shift_hdr(buf: &mut [u8], field: usize, delta: i64) -> Result<(), FdtError> {
    let v = hdr(buf, field) as i64 + delta;
    if v < 0 {
        return Err(FdtError::Internal);
    }
    set_hdr(buf, field, v as usize)
}

fn align4(x: usize) -> usize {
    (x + 3) & !3
}

/// `fdt_splice_()`: open a gap of `newlen` bytes at `pos`, after removing
/// `oldlen` bytes there. Everything from `pos + oldlen` to the end of the data
/// block moves. The new bytes are not written.
fn splice(buf: &mut [u8], pos: usize, oldlen: usize, newlen: usize) -> Result<(), FdtError> {
    let totalsize = hdr(buf, H_TOTALSIZE) as usize;
    let dsize = hdr(buf, H_OFF_DT_STRINGS) as usize + hdr(buf, H_SIZE_DT_STRINGS) as usize;
    if totalsize > buf.len() || dsize > totalsize {
        return Err(FdtError::Truncated);
    }
    if pos + oldlen > dsize {
        return Err(FdtError::BadOffset);
    }
    if dsize - oldlen + newlen > totalsize {
        return Err(FdtError::NoSpace);
    }
    buf.copy_within(pos + oldlen..dsize, pos + newlen);
    Ok(())
}

/// `fdt_splice_mem_rsv_()`: splice the reservation map and move the blocks
/// after it.
fn splice_mem_rsv(buf: &mut [u8], pos: usize, oldn: usize, newn: usize) -> Result<(), FdtError> {
    let delta = (newn as i64 - oldn as i64) * RSV_ENTRY as i64;
    splice(buf, pos, oldn * RSV_ENTRY, newn * RSV_ENTRY)?;
    shift_hdr(buf, H_OFF_DT_STRUCT, delta)?;
    shift_hdr(buf, H_OFF_DT_STRINGS, delta)
}

/// `fdt_splice_struct_()`: splice the structure block and move the strings.
fn splice_struct(buf: &mut [u8], pos: usize, oldlen: usize, newlen: usize) -> Result<(), FdtError> {
    let delta = newlen as i64 - oldlen as i64;
    splice(buf, pos, oldlen, newlen)?;
    shift_hdr(buf, H_SIZE_DT_STRUCT, delta)?;
    shift_hdr(buf, H_OFF_DT_STRINGS, delta)
}

/// `fdt_splice_string_()`: grow the strings block at its end.
fn splice_string(buf: &mut [u8], newlen: usize) -> Result<(), FdtError> {
    let p = hdr(buf, H_OFF_DT_STRINGS) as usize + hdr(buf, H_SIZE_DT_STRINGS) as usize;
    splice(buf, p, 0, newlen)?;
    shift_hdr(buf, H_SIZE_DT_STRINGS, newlen as i64)
}

/// `fdt_del_last_string_()`: undo a string added in the same operation.
fn del_last_string(buf: &mut [u8], len: usize) -> Result<(), FdtError> {
    shift_hdr(buf, H_SIZE_DT_STRINGS, -(len as i64))
}

/// `fdt_find_add_string_()`: the strings-block offset of `s`, adding it at the
/// end if it is not there. The flag says whether it was added.
fn find_add_string(buf: &mut [u8], s: &[u8]) -> Result<(usize, bool), FdtError> {
    let off = hdr(buf, H_OFF_DT_STRINGS) as usize;
    let size = hdr(buf, H_SIZE_DT_STRINGS) as usize;
    if let Some(i) = find_string_len(buf.get(off..off + size).ok_or(FdtError::Truncated)?, s) {
        return Ok((i, false));
    }
    splice_string(buf, s.len() + 1)?;
    put(buf, off + size, s)?;
    put(buf, off + size + s.len(), &[0])?;
    Ok((size, true))
}

/// `nop_region_()`: overwrite a region with NOP tags, one per 4 bytes.
fn nop_region(buf: &mut [u8], start: usize, len: usize) -> Result<(), FdtError> {
    let mut i = 0;
    while i < len {
        put(buf, start + i, &FDT_NOP.to_be_bytes())?;
        i += FDT_TAGSIZE;
    }
    Ok(())
}

/// Copy `len` bytes of `src` to `dst` (`copy_block`).
fn copy_block(dst: &mut [u8], dst_off: usize, src: &[u8], src_off: usize, len: usize) -> Result<(), FdtError> {
    let s = src
        .get(src_off..src_off.checked_add(len).ok_or(FdtError::Truncated)?)
        .ok_or(FdtError::Truncated)?;
    put(dst, dst_off, s)
}

/// `fdt_packblocks_()` on one buffer: move the three blocks, in order, to the
/// packed layout. Each move is a `memmove`, so the blocks may overlap.
fn pack_blocks_in_place(
    buf: &mut [u8],
    old: [usize; 3],
    sizes: [usize; 3],
) -> Result<(), FdtError> {
    let [old_mem, old_struct, old_strings] = old;
    let [mem_size, struct_size, strings_size] = sizes;
    let mem_off = FDT_V17_SIZE;
    let struct_off = mem_off + mem_size;
    let strings_off = struct_off + struct_size;
    if strings_off + strings_size > buf.len() {
        return Err(FdtError::NoSpace);
    }
    buf.copy_within(old_mem..old_mem + mem_size, mem_off);
    set_hdr(buf, H_OFF_MEM_RSVMAP, mem_off)?;
    buf.copy_within(old_struct..old_struct + struct_size, struct_off);
    set_hdr(buf, H_OFF_DT_STRUCT, struct_off)?;
    set_hdr(buf, H_SIZE_DT_STRUCT, struct_size)?;
    buf.copy_within(old_strings..old_strings + strings_size, strings_off);
    set_hdr(buf, H_OFF_DT_STRINGS, strings_off)?;
    set_hdr(buf, H_SIZE_DT_STRINGS, strings_size)
}

/// `fdt_open_into()`: copy a finished tree into `dst`, packing the blocks if
/// they are out of order. `src` and `dst` are separate buffers here, so the C
/// in-place form is `open_into` into a second buffer.
pub fn open_into(src: &[u8], dst: &mut [u8]) -> Result<(), FdtError> {
    let v = Fdt::new(src)?;
    let mem_rsv_size = (v.num_mem_rsv()? + 1) * RSV_ENTRY;
    let struct_size = match v.version() {
        ver if ver >= 17 => v.size_dt_struct(),
        16 => {
            let mut off = 0;
            loop {
                let (tok, next) = v.next_tag(off)?;
                off = next;
                if tok == Token::End {
                    break;
                }
            }
            off
        }
        _ => return Err(FdtError::BadVersion),
    };

    if !v.blocks_misordered(mem_rsv_size, struct_size) {
        v.move_to(dst)?;
        set_hdr(dst, H_VERSION, 17)?;
        set_hdr(dst, H_SIZE_DT_STRUCT, struct_size)?;
        set_hdr(dst, H_TOTALSIZE, dst.len())?;
        return Ok(());
    }

    let strings_size = v.size_dt_strings();
    let newsize = FDT_V17_SIZE + mem_rsv_size + struct_size + strings_size;
    if dst.len() < newsize {
        return Err(FdtError::NoSpace);
    }
    let mem_off = FDT_V17_SIZE;
    let struct_off = mem_off + mem_rsv_size;
    let strings_off = struct_off + struct_size;
    copy_block(dst, mem_off, src, v.off_mem_rsvmap(), mem_rsv_size)?;
    copy_block(dst, struct_off, src, v.off_dt_struct(), struct_size)?;
    copy_block(dst, strings_off, src, v.off_dt_strings(), strings_size)?;
    let len = dst.len();
    set_hdr(dst, H_MAGIC, FDT_MAGIC as usize)?;
    set_hdr(dst, H_TOTALSIZE, len)?;
    set_hdr(dst, H_VERSION, 17)?;
    set_hdr(dst, H_LAST_COMP, FDT_LAST_COMPATIBLE_VERSION)?;
    put(dst, H_BOOT_CPUID, &v.boot_cpuid_phys().to_be_bytes())?;
    set_hdr(dst, H_OFF_MEM_RSVMAP, mem_off)?;
    set_hdr(dst, H_OFF_DT_STRUCT, struct_off)?;
    set_hdr(dst, H_SIZE_DT_STRUCT, struct_size)?;
    set_hdr(dst, H_OFF_DT_STRINGS, strings_off)?;
    set_hdr(dst, H_SIZE_DT_STRINGS, strings_size)
}

/// A finished device tree being edited in place: `fdt_*()` in `fdt_rw.c` and
/// `fdt_wip.c`. Every edit keeps the blocks in order and the header current.
#[derive(Debug)]
pub struct FdtMut<'a> {
    buf: &'a mut [u8],
}

impl<'a> FdtMut<'a> {
    /// `FDT_RW_PROBE()`: check that the tree is version 17 or later, with its
    /// blocks in order, and downgrade a version newer than this code knows.
    pub fn open(buf: &'a mut [u8]) -> Result<Self, FdtError> {
        let totalsize = ro_probe(buf)?;
        if totalsize < FDT_V17_SIZE || totalsize > buf.len() {
            return Err(FdtError::Truncated);
        }
        {
            let v = Fdt { buf: &buf[..totalsize] };
            if v.version() < 17 {
                return Err(FdtError::BadVersion);
            }
            if v.blocks_misordered(RSV_ENTRY, v.size_dt_struct()) {
                return Err(FdtError::BadLayout);
            }
        }
        if hdr(buf, H_VERSION) as usize > FDT_LAST_SUPPORTED_VERSION as usize {
            set_hdr(buf, H_VERSION, FDT_LAST_SUPPORTED_VERSION as usize)?;
        }
        Ok(Self { buf })
    }

    /// The read-only view of the tree being edited.
    pub fn as_fdt(&self) -> Fdt<'_> {
        self.view()
    }

    fn view(&self) -> Fdt<'_> {
        let totalsize = hdr(self.buf, H_TOTALSIZE) as usize;
        Fdt { buf: &self.buf[..totalsize] }
    }

    /// The structure-block offset of a property and its value length.
    fn prop_info(&self, node: usize, name: &[u8]) -> Result<(usize, usize), FdtError> {
        let v = self.view();
        let (poff, prop) = v.find_property(node, name)?;
        Ok((poff, prop.data.len()))
    }

    /// `fdt_add_mem_rsv()`: add a reservation at the end of the list.
    pub fn add_mem_rsv(&mut self, address: u64, size: u64) -> Result<(), FdtError> {
        let (n, off) = {
            let v = self.view();
            (v.num_mem_rsv()?, v.off_mem_rsvmap())
        };
        let pos = off + n * RSV_ENTRY;
        splice_mem_rsv(self.buf, pos, 0, 1)?;
        put(self.buf, pos, &address.to_be_bytes())?;
        put(self.buf, pos + 8, &size.to_be_bytes())
    }

    /// `fdt_del_mem_rsv()`: remove the `n`th reservation.
    pub fn del_mem_rsv(&mut self, n: usize) -> Result<(), FdtError> {
        let (num, off) = {
            let v = self.view();
            (v.num_mem_rsv()?, v.off_mem_rsvmap())
        };
        if n >= num {
            return Err(FdtError::NotFound);
        }
        splice_mem_rsv(self.buf, off + n * RSV_ENTRY, 1, 0)
    }

    /// `fdt_set_name()`.
    pub fn set_name(&mut self, node: usize, name: &[u8]) -> Result<(), FdtError> {
        let (oldlen, struct_off) = {
            let v = self.view();
            (v.get_name(node)?.len(), v.off_dt_struct())
        };
        let namepos = struct_off + node + FDT_TAGSIZE;
        splice_struct(self.buf, namepos, align4(oldlen + 1), align4(name.len() + 1))?;
        put(self.buf, namepos, name)?;
        zero(self.buf, namepos + name.len(), align4(name.len() + 1) - name.len())
    }

    /// `fdt_resize_property_()`: a property that exists gets a new length, and
    /// its data offset in the buffer comes back.
    fn resize_property(&mut self, node: usize, name: &[u8], len: usize) -> Result<usize, FdtError> {
        let (poff, oldlen) = self.prop_info(node, name)?;
        let struct_off = self.view().off_dt_struct();
        let data = struct_off + poff + PROP_HEADER;
        splice_struct(self.buf, data, align4(oldlen), align4(len))?;
        put(self.buf, data - 8, &(len as u32).to_be_bytes())?;
        zero_pad(self.buf, data, len)?;
        Ok(data)
    }

    /// `fdt_add_property_()`: a new property at the end of a node's
    /// properties, with its value zeroed, and the data offset comes back.
    fn add_property(&mut self, node: usize, name: &[u8], len: usize) -> Result<usize, FdtError> {
        let next = self.view().check_node_offset(node)?;
        let (namestroff, allocated) = find_add_string(self.buf, name)?;
        let struct_off = self.view().off_dt_struct();
        let pos = struct_off + next;
        let proplen = PROP_HEADER + align4(len);
        if let Err(e) = splice_struct(self.buf, pos, 0, proplen) {
            if allocated {
                del_last_string(self.buf, name.len() + 1)?;
            }
            return Err(e);
        }
        zero(self.buf, pos, proplen)?;
        put(self.buf, pos, &FDT_PROP.to_be_bytes())?;
        put(self.buf, pos + 4, &(len as u32).to_be_bytes())?;
        put(self.buf, pos + 8, &(namestroff as u32).to_be_bytes())?;
        Ok(pos + PROP_HEADER)
    }

    /// `fdt_setprop_placeholder()`: the value of a property, sized to `len`
    /// bytes and ready to be written. The bytes are zero.
    pub fn setprop_placeholder(&mut self, node: usize, name: &[u8], len: usize) -> Result<&mut [u8], FdtError> {
        let pos = match self.resize_property(node, name, len) {
            Err(FdtError::NotFound) => self.add_property(node, name, len)?,
            r => r?,
        };
        Ok(&mut self.buf[pos..pos + len])
    }

    /// `fdt_setprop()`.
    pub fn setprop(&mut self, node: usize, name: &[u8], val: &[u8]) -> Result<(), FdtError> {
        self.setprop_placeholder(node, name, val.len())?.copy_from_slice(val);
        Ok(())
    }

    /// `fdt_setprop_inplace_namelen_partial()`: overwrite part of a value that
    /// is already long enough, without resizing.
    pub fn setprop_inplace_partial(&mut self, node: usize, name: &[u8], idx: usize, val: &[u8]) -> Result<(), FdtError> {
        let (poff, proplen) = self.prop_info(node, name)?;
        if proplen < val.len() + idx {
            return Err(FdtError::NoSpace);
        }
        let struct_off = self.view().off_dt_struct();
        put(self.buf, struct_off + poff + PROP_HEADER + idx, val)
    }

    /// `fdt_setprop_inplace()`: overwrite a value of exactly the same length.
    pub fn setprop_inplace(&mut self, node: usize, name: &[u8], val: &[u8]) -> Result<(), FdtError> {
        let (_, proplen) = self.prop_info(node, name)?;
        if proplen != val.len() {
            return Err(FdtError::NoSpace);
        }
        self.setprop_inplace_partial(node, name, 0, val)
    }

    /// `fdt_appendprop()`: add bytes to the end of a value, creating the
    /// property if it is not there.
    pub fn appendprop(&mut self, node: usize, name: &[u8], val: &[u8]) -> Result<(), FdtError> {
        match self.prop_info(node, name) {
            Ok((poff, oldlen)) => {
                let newlen = oldlen + val.len();
                let struct_off = self.view().off_dt_struct();
                let data = struct_off + poff + PROP_HEADER;
                splice_struct(self.buf, data, align4(oldlen), align4(newlen))?;
                put(self.buf, data - 8, &(newlen as u32).to_be_bytes())?;
                put(self.buf, data + oldlen, val)?;
                zero_pad(self.buf, data, newlen)
            }
            Err(_) => {
                let pos = self.add_property(node, name, val.len())?;
                put(self.buf, pos, val)
            }
        }
    }

    /// `fdt_appendprop_addrrange()`: append an address and a size as cells,
    /// using the parent's `#address-cells` and `#size-cells`.
    pub fn appendprop_addrrange(
        &mut self,
        parent: usize,
        node: usize,
        name: &[u8],
        addr: u64,
        size: u64,
    ) -> Result<(), FdtError> {
        let (addr_cells, size_cells) = {
            let v = self.view();
            (v.address_cells(parent)?, v.size_cells(parent)?)
        };
        let mut data = [0u8; 16];
        match addr_cells {
            1 => {
                if addr > u32::MAX as u64 || (u32::MAX as u64 + 1 - addr) < size {
                    return Err(FdtError::BadValue);
                }
                data[..4].copy_from_slice(&(addr as u32).to_be_bytes());
            }
            2 => data[..8].copy_from_slice(&addr.to_be_bytes()),
            _ => return Err(FdtError::BadNcells),
        }
        let p = addr_cells as usize * 4;
        match size_cells {
            1 => {
                if size > u32::MAX as u64 {
                    return Err(FdtError::BadValue);
                }
                data[p..p + 4].copy_from_slice(&(size as u32).to_be_bytes());
            }
            2 => data[p..p + 8].copy_from_slice(&size.to_be_bytes()),
            _ => return Err(FdtError::BadNcells),
        }
        let n = (addr_cells + size_cells) as usize * 4;
        self.appendprop(node, name, &data[..n])
    }

    /// `fdt_delprop()`.
    pub fn delprop(&mut self, node: usize, name: &[u8]) -> Result<(), FdtError> {
        let (poff, len) = self.prop_info(node, name)?;
        let struct_off = self.view().off_dt_struct();
        splice_struct(self.buf, struct_off + poff, PROP_HEADER + align4(len), 0)
    }

    /// `fdt_add_subnode()`: a new child, placed after the parent's properties
    /// and before its other children. Returns the new node's offset.
    pub fn add_subnode(&mut self, parent: usize, name: &[u8]) -> Result<usize, FdtError> {
        let (offset, nodelen) = {
            let v = self.view();
            match v.subnode_offset(parent, name) {
                Ok(_) => return Err(FdtError::Exists),
                Err(FdtError::NotFound) => {}
                Err(e) => return Err(e),
            }
            let (tok, after) = v.next_tag(parent)?;
            if tok != Token::BeginNode {
                return Err(FdtError::Internal);
            }
            let mut offset = after;
            loop {
                let (t, next) = v.next_tag(offset)?;
                if t != Token::Prop && t != Token::Nop {
                    break;
                }
                offset = next;
            }
            (offset, FDT_TAGSIZE + align4(name.len() + 1) + FDT_TAGSIZE)
        };
        let pos = self.view().off_dt_struct() + offset;
        splice_struct(self.buf, pos, 0, nodelen)?;
        zero(self.buf, pos, nodelen)?;
        put(self.buf, pos, &FDT_BEGIN_NODE.to_be_bytes())?;
        put(self.buf, pos + FDT_TAGSIZE, name)?;
        put(self.buf, pos + nodelen - FDT_TAGSIZE, &FDT_END_NODE.to_be_bytes())?;
        Ok(offset)
    }

    /// `fdt_del_node()`.
    pub fn del_node(&mut self, node: usize) -> Result<(), FdtError> {
        let (struct_off, end) = {
            let v = self.view();
            (v.off_dt_struct(), v.node_end_offset(node)?)
        };
        splice_struct(self.buf, struct_off + node, end - node, 0)
    }

    /// `fdt_nop_property()`: turn a property into NOP tags.
    pub fn nop_property(&mut self, node: usize, name: &[u8]) -> Result<(), FdtError> {
        let (poff, len) = self.prop_info(node, name)?;
        let struct_off = self.view().off_dt_struct();
        nop_region(self.buf, struct_off + poff, PROP_HEADER + len)
    }

    /// `fdt_nop_node()`: turn a node and its children into NOP tags.
    pub fn nop_node(&mut self, node: usize) -> Result<(), FdtError> {
        let (struct_off, end) = {
            let v = self.view();
            (v.off_dt_struct(), v.node_end_offset(node)?)
        };
        nop_region(self.buf, struct_off + node, end - node)
    }

    /// `fdt_pack()`: remove the free space between blocks, and shrink the
    /// header's `totalsize` to the data.
    pub fn pack(&mut self) -> Result<(), FdtError> {
        let (num, old_mem, struct_size, strings_size, old_struct, old_strings) = {
            let v = self.view();
            (
                v.num_mem_rsv()?,
                v.off_mem_rsvmap(),
                v.size_dt_struct(),
                v.size_dt_strings(),
                v.off_dt_struct(),
                v.off_dt_strings(),
            )
        };
        let mem_rsv_size = (num + 1) * RSV_ENTRY;
        pack_blocks_in_place(
            self.buf,
            [old_mem, old_struct, old_strings],
            [mem_rsv_size, struct_size, strings_size],
        )?;
        let total = hdr(self.buf, H_OFF_DT_STRINGS) as usize + strings_size;
        set_hdr(self.buf, H_TOTALSIZE, total)
    }
}

/// A device tree being built in order: `fdt_*()` in `fdt_sw.c`. The reservation
/// map is written first, then the structure, then the strings are moved to
/// the end by [`FdtSw::finish`].
#[derive(Debug)]
pub struct FdtSw<'a> {
    buf: &'a mut [u8],
}

impl<'a> FdtSw<'a> {
    /// `fdt_create_with_flags()`: start an empty tree in `buf`, which is zeroed.
    pub fn create(buf: &'a mut [u8], flags: u32) -> Result<Self, FdtError> {
        if buf.len() < HDR_SIZE_ALIGNED {
            return Err(FdtError::NoSpace);
        }
        if flags & !FDT_CREATE_FLAG_NO_NAME_DEDUP != 0 {
            return Err(FdtError::BadFlags);
        }
        buf.fill(0);
        let total = buf.len();
        set_hdr(buf, H_MAGIC, FDT_SW_MAGIC as usize)?;
        set_hdr(buf, H_VERSION, FDT_LAST_SUPPORTED_VERSION as usize)?;
        set_hdr(buf, H_LAST_COMP, flags as usize)?;
        set_hdr(buf, H_TOTALSIZE, total)?;
        set_hdr(buf, H_OFF_MEM_RSVMAP, HDR_SIZE_ALIGNED)?;
        set_hdr(buf, H_OFF_DT_STRUCT, HDR_SIZE_ALIGNED)?;
        set_hdr(buf, H_OFF_DT_STRINGS, 0)?;
        Ok(Self { buf })
    }

    /// Continue a tree that [`FdtSw::create`] started. Only the magic is checked.
    pub fn open(buf: &'a mut [u8]) -> Result<Self, FdtError> {
        if buf.len() < FDT_V17_SIZE {
            return Err(FdtError::Truncated);
        }
        match hdr(buf, H_MAGIC) {
            FDT_SW_MAGIC => Ok(Self { buf }),
            FDT_MAGIC => Err(FdtError::BadState),
            _ => Err(FdtError::BadMagic),
        }
    }

    /// `fdt_sw_probe_memrsv_()`: still writing the reservation map.
    fn probe_memrsv(&self) -> Result<(), FdtError> {
        if hdr(self.buf, H_OFF_DT_STRINGS) != 0 {
            return Err(FdtError::BadState);
        }
        Ok(())
    }

    /// `fdt_sw_probe_struct_()`: writing the structure block.
    fn probe_struct(&self) -> Result<(), FdtError> {
        if hdr(self.buf, H_OFF_DT_STRINGS) as usize != hdr(self.buf, H_TOTALSIZE) as usize {
            return Err(FdtError::BadState);
        }
        Ok(())
    }

    /// `fdt_grab_space_()`: reserve `len` bytes at the end of the structure
    /// block, returning their buffer offset.
    fn grab_space(&mut self, len: usize) -> Option<usize> {
        let total = hdr(self.buf, H_TOTALSIZE) as usize;
        let struct_off = hdr(self.buf, H_OFF_DT_STRUCT) as usize;
        let size_struct = hdr(self.buf, H_SIZE_DT_STRUCT) as usize;
        let size_strings = hdr(self.buf, H_SIZE_DT_STRINGS) as usize;
        let offset = size_struct;
        let spaceleft = total.checked_sub(struct_off)?.checked_sub(size_strings)?;
        if offset.checked_add(len)? > spaceleft {
            return None;
        }
        set_hdr(self.buf, H_SIZE_DT_STRUCT, offset + len).ok()?;
        Some(struct_off + offset)
    }

    /// `fdt_add_reservemap_entry()`.
    pub fn add_reservemap_entry(&mut self, addr: u64, size: u64) -> Result<(), FdtError> {
        self.probe_memrsv()?;
        let offset = hdr(self.buf, H_OFF_DT_STRUCT) as usize;
        if offset + RSV_ENTRY > hdr(self.buf, H_TOTALSIZE) as usize {
            return Err(FdtError::NoSpace);
        }
        put(self.buf, offset, &addr.to_be_bytes())?;
        put(self.buf, offset + 8, &size.to_be_bytes())?;
        set_hdr(self.buf, H_OFF_DT_STRUCT, offset + RSV_ENTRY)
    }

    /// `fdt_finish_reservemap()`: end the reservation map and start the structure.
    pub fn finish_reservemap(&mut self) -> Result<(), FdtError> {
        self.add_reservemap_entry(0, 0)?;
        let total = hdr(self.buf, H_TOTALSIZE) as usize;
        set_hdr(self.buf, H_OFF_DT_STRINGS, total)
    }

    /// `fdt_begin_node()`.
    pub fn begin_node(&mut self, name: &[u8]) -> Result<(), FdtError> {
        self.probe_struct()?;
        let namelen = name.len() + 1;
        let pos = self
            .grab_space(FDT_TAGSIZE + align4(namelen))
            .ok_or(FdtError::NoSpace)?;
        put(self.buf, pos, &FDT_BEGIN_NODE.to_be_bytes())?;
        put(self.buf, pos + FDT_TAGSIZE, name)
    }

    /// `fdt_end_node()`.
    pub fn end_node(&mut self) -> Result<(), FdtError> {
        self.probe_struct()?;
        let pos = self.grab_space(FDT_TAGSIZE).ok_or(FdtError::NoSpace)?;
        put(self.buf, pos, &FDT_END_NODE.to_be_bytes())
    }

    /// `fdt_add_string_()`: append a string at the strings block's start, which
    /// grows downward from the end. Returns its offset from the end, or 0 if
    /// there is no room.
    fn add_string(&mut self, s: &[u8]) -> Result<i32, FdtError> {
        let total = hdr(self.buf, H_TOTALSIZE) as usize;
        let strtabsize = hdr(self.buf, H_SIZE_DT_STRINGS) as usize;
        let struct_top = hdr(self.buf, H_OFF_DT_STRUCT) as usize + hdr(self.buf, H_SIZE_DT_STRUCT) as usize;
        let len = s.len() + 1;
        let offset = strtabsize + len;
        if offset > total || total - offset < struct_top {
            return Ok(0);
        }
        let pos = total - offset;
        put(self.buf, pos, s)?;
        put(self.buf, pos + s.len(), &[0])?;
        set_hdr(self.buf, H_SIZE_DT_STRINGS, strtabsize + len)?;
        Ok(-(offset as i32))
    }

    /// `fdt_find_add_string_()` for the sequential form: a negative offset from
    /// the end of the buffer, and whether it was added.
    fn find_add_string(&mut self, s: &[u8]) -> Result<(i32, bool), FdtError> {
        let total = hdr(self.buf, H_TOTALSIZE) as usize;
        let size = hdr(self.buf, H_SIZE_DT_STRINGS) as usize;
        let region = self.buf.get(total - size..total).ok_or(FdtError::Truncated)?;
        if let Some(i) = find_string_len(region, s) {
            return Ok(((i as i64 - size as i64) as i32, false));
        }
        Ok((self.add_string(s)?, true))
    }

    /// `fdt_property_placeholder()`: a property whose `len` bytes are zero and
    /// ready to be written.
    pub fn property_placeholder(&mut self, name: &[u8], len: usize) -> Result<&mut [u8], FdtError> {
        self.probe_struct()?;
        let flags = hdr(self.buf, H_LAST_COMP);
        let (nameoff, allocated) = if flags & FDT_CREATE_FLAG_NO_NAME_DEDUP != 0 {
            (self.add_string(name)?, true)
        } else {
            self.find_add_string(name)?
        };
        if nameoff == 0 {
            return Err(FdtError::NoSpace);
        }
        let proplen = PROP_HEADER + align4(len);
        let pos = match self.grab_space(proplen) {
            Some(p) => p,
            None => {
                if allocated {
                    del_last_string(self.buf, name.len() + 1)?;
                }
                return Err(FdtError::NoSpace);
            }
        };
        zero(self.buf, pos, proplen)?;
        put(self.buf, pos, &FDT_PROP.to_be_bytes())?;
        put(self.buf, pos + 4, &(len as u32).to_be_bytes())?;
        put(self.buf, pos + 8, &(nameoff as u32).to_be_bytes())?;
        Ok(&mut self.buf[pos + PROP_HEADER..pos + PROP_HEADER + len])
    }

    /// `fdt_property()`.
    pub fn property(&mut self, name: &[u8], val: &[u8]) -> Result<(), FdtError> {
        self.property_placeholder(name, val.len())?.copy_from_slice(val);
        Ok(())
    }

    /// `fdt_finish()`: close the tree, move the strings after the structure,
    /// and fix the name offsets and the header into a finished tree.
    pub fn finish(mut self) -> Result<(), FdtError> {
        self.probe_struct()?;
        let end = self.grab_space(FDT_TAGSIZE).ok_or(FdtError::NoSpace)?;
        put(self.buf, end, &FDT_END.to_be_bytes())?;

        let total = hdr(self.buf, H_TOTALSIZE) as usize;
        let size_strings = hdr(self.buf, H_SIZE_DT_STRINGS) as usize;
        let struct_off = hdr(self.buf, H_OFF_DT_STRUCT) as usize;
        let size_struct = hdr(self.buf, H_SIZE_DT_STRUCT) as usize;
        let old_str = total - size_strings;
        let new_str = struct_off + size_struct;
        self.buf.copy_within(old_str..old_str + size_strings, new_str);
        set_hdr(self.buf, H_OFF_DT_STRINGS, new_str)?;

        let mut offset = 0;
        loop {
            let (tok, next) = Fdt { buf: &self.buf[..total] }.next_tag(offset)?;
            if tok == Token::End {
                break;
            }
            if tok == Token::Prop {
                let p = struct_off + offset;
                let nameoff = be32(self.buf, p + 8).ok_or(FdtError::BadStructure)?;
                let fixed = nameoff.wrapping_add(size_strings as u32);
                put(self.buf, p + 8, &fixed.to_be_bytes())?;
            }
            offset = next;
        }

        set_hdr(self.buf, H_TOTALSIZE, new_str + size_strings)?;
        set_hdr(self.buf, H_LAST_COMP, FDT_LAST_COMPATIBLE_VERSION)?;
        set_hdr(self.buf, H_MAGIC, FDT_MAGIC as usize)
    }
}

/// `fdt_sw_resize()`: copy an unfinished tree into a bigger or smaller buffer.
/// The source and destination are separate buffers, so the C in-place form is
/// not needed.
pub fn sw_resize(src: &[u8], dst: &mut [u8]) -> Result<(), FdtError> {
    if src.len() < FDT_V17_SIZE {
        return Err(FdtError::Truncated);
    }
    match hdr(src, H_MAGIC) {
        FDT_SW_MAGIC => {}
        FDT_MAGIC => return Err(FdtError::BadState),
        _ => return Err(FdtError::BadMagic),
    }
    let total = hdr(src, H_TOTALSIZE) as usize;
    if total > src.len() {
        return Err(FdtError::Truncated);
    }
    let head = hdr(src, H_OFF_DT_STRUCT) as usize + hdr(src, H_SIZE_DT_STRUCT) as usize;
    let tail = hdr(src, H_SIZE_DT_STRINGS) as usize;
    if head + tail > total {
        return Err(FdtError::Internal);
    }
    let dlen = dst.len();
    if head + tail > dlen {
        return Err(FdtError::NoSpace);
    }
    copy_block(dst, 0, src, 0, head)?;
    copy_block(dst, dlen - tail, src, total - tail, tail)?;
    set_hdr(dst, H_TOTALSIZE, dlen)?;
    if hdr(dst, H_OFF_DT_STRINGS) != 0 {
        set_hdr(dst, H_OFF_DT_STRINGS, dlen)?;
    }
    Ok(())
}

/// `fdt_create_empty_tree()`: a tree with a root node and nothing else.
pub fn create_empty_tree(buf: &mut [u8]) -> Result<(), FdtError> {
    let bufsize = buf.len();
    {
        let mut sw = FdtSw::create(&mut *buf, 0)?;
        sw.finish_reservemap()?;
        sw.begin_node(b"")?;
        sw.end_node()?;
        sw.finish()?;
    }
    let struct_size = {
        let v = Fdt::new(&*buf)?;
        let mem_rsv_size = (v.num_mem_rsv()? + 1) * RSV_ENTRY;
        let struct_size = v.size_dt_struct();
        // A fresh tree is always in order, so C's repack path is never taken.
        if v.blocks_misordered(mem_rsv_size, struct_size) {
            return Err(FdtError::Internal);
        }
        struct_size
    };
    set_hdr(buf, H_VERSION, 17)?;
    set_hdr(buf, H_SIZE_DT_STRUCT, struct_size)?;
    set_hdr(buf, H_TOTALSIZE, bufsize)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strerror_matches_the_c_table() {
        assert_eq!(strerror(0), "<no error>");
        assert_eq!(strerror(5), "<valid offset/length>");
        assert_eq!(strerror(-1), "FDT_ERR_NOTFOUND");
        assert_eq!(strerror(-19), "FDT_ERR_ALIGNMENT");
        assert_eq!(strerror(-20), "<unknown error>");
        assert_eq!(strerror(i32::MIN), "<unknown error>");
    }

    #[test]
    fn find_string_matches_whole_entries() {
        let tab = b"abc\0ab\0";
        assert_eq!(find_string_len(tab, b"ab"), Some(4));
        assert_eq!(find_string_len(tab, b"abc"), Some(0));
        assert_eq!(find_string_len(tab, b"a"), None);
    }

    #[test]
    fn stringlist_contains_matches_whole_strings() {
        let list = b"foo\0bar\0";
        assert!(stringlist_contains(list, b"bar"));
        assert!(!stringlist_contains(list, b"ba"));
        assert!(!stringlist_contains(list, b"baz"));
    }
}

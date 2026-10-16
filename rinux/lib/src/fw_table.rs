// SPDX-License-Identifier: GPL-2.0-or-later
//! Generic walker over ACPI-style and CDAT subtables.
//!
//! Ported from `lib/fw_table.c` and `include/linux/fw_table.h`.
//!
//! Copyright (C) 2001 Paul Diefenbaugh <paul.s.diefenbaugh@intel.com>
//! Copyright (C) 2023 Intel Corp.
//!
//! The C code reads subtables through a union of header types and raw
//! pointers. Here the table is a byte slice and each subtable is an offset
//! into it. Two header layouts are ported: the common ACPI subtable header
//! (`u8 type`, `u8 length`) and the CDAT subtable header (`u8 type`, a
//! reserved byte, `u16 length` little-endian). The HMAT, PRMT and CEDT
//! layouts are not ported and return `EOPNOTSUPP`.
//!
//! The C handler receives the table end as well as the entry. Here the
//! handler receives the entry's own bytes, and an entry that runs past the
//! end of the table is `EINVAL` rather than an out-of-bounds read.

use crate::errno::{EINVAL, EOPNOTSUPP, Result};

/// Size of the CDAT table header (`struct acpi_table_cdat`).
const CDAT_TABLE_HEADER_LEN: usize = 16;

/// `ACPI_SIG_CDAT`, `ACPI_SIG_HMAT`, `ACPI_SIG_PRMT`, `ACPI_SIG_CEDT`.
const SIG_CDAT: &[u8] = b"CDAT";
const SIG_HMAT: &[u8] = b"HMAT";
const SIG_PRMT: &[u8] = b"PRMT";
const SIG_CEDT: &[u8] = b"CEDT";

/// `enum acpi_subtable_type`, for the layouts that are ported.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SubtableType {
    /// `ACPI_SUBTABLE_COMMON`: `u8 type`, `u8 length`.
    Common,
    /// `CDAT_SUBTABLE`: `u8 type`, `u8 reserved`, `u16 length` (LE).
    Cdat,
}

impl SubtableType {
    /// `acpi_get_subtable_type()`: the layout for a table signature.
    fn from_signature(id: &[u8]) -> Result<Self> {
        if id.starts_with(SIG_CDAT) {
            Ok(Self::Cdat)
        } else if id.starts_with(SIG_HMAT) || id.starts_with(SIG_PRMT) || id.starts_with(SIG_CEDT) {
            Err(EOPNOTSUPP)
        } else {
            Ok(Self::Common)
        }
    }

    /// `acpi_get_subtable_header_length()`.
    fn header_len(self) -> usize {
        match self {
            Self::Common => 2,
            Self::Cdat => 4,
        }
    }

    /// `acpi_get_entry_type()`: `entry` starts at the subtable header.
    fn entry_type(self, entry: &[u8]) -> u8 {
        entry[0]
    }

    /// `acpi_get_entry_length()`: `entry` starts at the subtable header.
    fn entry_length(self, entry: &[u8]) -> usize {
        match self {
            Self::Common => entry[1] as usize,
            Self::Cdat => u16::from_le_bytes([entry[2], entry[3]]) as usize,
        }
    }

    /// `acpi_table_get_length()`: the length field of the root table header.
    fn table_length(self, table: &[u8]) -> Result<usize> {
        // ACPI puts the length at offset 4 after the signature. CDAT puts it
        // first.
        let at = match self {
            Self::Common => 4,
            Self::Cdat => 0,
        };
        let bytes = table.get(at..at + 4).ok_or(EINVAL)?;
        Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as usize)
    }
}

/// `struct acpi_subtable_proc`: one entry type to look for, with its handler.
pub struct SubtableProc<'a> {
    /// The entry type this handler matches.
    pub id: u8,
    /// How many entries of this type were found, including those past
    /// `max_entries`.
    pub count: usize,
    /// Called with the bytes of each matching entry, up to `max_entries`.
    /// An error stops the walk with `EINVAL`, as the C handler's non-zero
    /// return does.
    pub handler: &'a mut dyn FnMut(&[u8]) -> Result<()>,
}

/// `acpi_parse_entries_array()`: for each entry, find the first `procs`
/// entry whose type matches and run its handler on it.
///
/// `table_size` is the offset of the first subtable, which is the root table
/// header's size. `max_length` caps the table length when it is non-zero.
/// `max_entries` caps the handler calls when it is non-zero. The return value
/// is the number of matching entries, counting those past `max_entries`.
///
/// Returns `EINVAL` for a table that is shorter than its own length, for a
/// zero-length entry, for an entry that runs past the table, and for a
/// handler error.
pub fn acpi_parse_entries_array(
    id: &[u8],
    table_size: usize,
    table: &[u8],
    max_length: usize,
    procs: &mut [SubtableProc<'_>],
    max_entries: usize,
) -> Result<usize> {
    let kind = SubtableType::from_signature(id)?;
    let mut table_len = kind.table_length(table)?;
    if max_length != 0 && max_length < table_len {
        table_len = max_length;
    }
    if table_len > table.len() {
        return Err(EINVAL);
    }

    let header_len = kind.header_len();
    let mut count = 0;
    let mut off = table_size;
    while off + header_len < table_len {
        let entry = &table[off..];
        let entry_type = kind.entry_type(entry);
        let entry_len = kind.entry_length(entry);
        if entry_len == 0 {
            return Err(EINVAL);
        }

        if let Some(proc) = procs.iter_mut().find(|p| p.id == entry_type) {
            if max_entries == 0 || count < max_entries {
                let bytes = table.get(off..off + entry_len).ok_or(EINVAL)?;
                (proc.handler)(bytes).map_err(|_| EINVAL)?;
            }
            proc.count += 1;
            count += 1;
        }

        off += entry_len;
    }

    Ok(count)
}

/// `cdat_table_parse()`: run `handler` on each CDAT subtable of `kind`.
///
/// `length` caps the table length when it is non-zero, as in the C function.
pub fn cdat_table_parse(
    kind: u8,
    handler: &mut dyn FnMut(&[u8]) -> Result<()>,
    table: &[u8],
    length: usize,
) -> Result<usize> {
    let mut procs = [SubtableProc {
        id: kind,
        count: 0,
        handler,
    }];
    acpi_parse_entries_array(
        SIG_CDAT,
        CDAT_TABLE_HEADER_LEN,
        table,
        length,
        &mut procs,
        0,
    )
}

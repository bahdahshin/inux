// SPDX-License-Identifier: GPL-2.0
//! Find a file in a `newc` cpio archive that is already in memory.
//!
//! Ported from `lib/earlycpio.c`, `find_cpio_data()`. The early boot code walks
//! the initramfs with this before any allocator exists. The C code walks the
//! archive with pointers; here positions are byte offsets into the archive,
//! and each header field is parsed from its eight (or six, for the magic)
//! hex digits, as C does. Names are returned as slices rather than copied into
//! a fixed buffer.

/// `MAX_CPIO_FILE_NAME`: the longest name C copies out.
pub const MAX_CPIO_FILE_NAME: usize = 4096;

/// `struct cpio_data`: a file found in the archive.
#[derive(Debug, PartialEq, Eq)]
pub struct CpioData<'a> {
    /// The name after the searched prefix, as C's `cd.name`.
    pub name: &'a [u8],
    /// The file contents.
    pub data: &'a [u8],
    /// Offset just past this entry, for the next search (`nextoff`).
    pub next_off: usize,
}

/// `cpio_header_len`: the magic (6 digits) and 13 fields of 8 digits, 110 bytes.
const CPIO_HEADER_LEN: usize = 8 * 14 - 2;
const MAGIC_NEWC: u32 = 0x0007_0701;
const MAGIC_CRC: u32 = 0x0007_0702;
const S_IFMT: u32 = 0o170000;
const S_IFREG: u32 = 0o100000;

fn align4(x: usize) -> usize {
    (x + 3) & !3
}

/// Parse the fields of a header at `p`: magic (6 digits), then 13 eight-digit
/// fields. Returns `None` on a non-hex digit or a short buffer.
fn parse_header(data: &[u8], p: usize) -> Option<[u32; 14]> {
    let mut ch = [0u32; 14];
    let mut pos = p;
    for (i, field) in ch.iter_mut().enumerate() {
        let digits = if i == 0 { 6 } else { 8 };
        let mut v: u32 = 0;
        for _ in 0..digits {
            let c = *data.get(pos)?;
            pos += 1;
            let x = c.wrapping_sub(b'0');
            let d = if x < 10 {
                u32::from(x)
            } else {
                let y = (c | 0x20).wrapping_sub(b'a');
                if y < 6 {
                    u32::from(y) + 10
                } else {
                    return None;
                }
            };
            v = (v << 4) + d;
        }
        *field = v;
    }
    Some(ch)
}

/// `find_cpio_data()`: the first regular file whose name starts with `path`.
/// Returns `None` when there is no such file or the archive is malformed.
pub fn find_cpio_data<'a>(path: &[u8], data: &'a [u8]) -> Option<CpioData<'a>> {
    let mut p = 0usize;
    while data.len().saturating_sub(p) > CPIO_HEADER_LEN {
        if data[p] == 0 {
            // Headers are 4-byte aligned; skip the zero padding.
            p += 4;
            continue;
        }
        let ch = parse_header(data, p)?;
        if ch[0] != MAGIC_NEWC && ch[0] != MAGIC_CRC {
            return None;
        }
        let namesize = ch[12] as usize;
        let filesize = ch[7] as usize;
        let mode = ch[2];
        let body = p + CPIO_HEADER_LEN;
        let dptr = align4(body + namesize);
        let nptr = align4(dptr + filesize);
        if nptr > data.len() || dptr < body || nptr < dptr {
            return None;
        }
        let name_field = &data[body..body + namesize];
        if mode & S_IFMT == S_IFREG && namesize >= path.len() && name_field.starts_with(path) {
            let rest = &name_field[path.len()..];
            let end = rest.iter().position(|&c| c == 0).unwrap_or(rest.len());
            let name = &rest[..end.min(MAX_CPIO_FILE_NAME - 1)];
            return Some(CpioData {
                name,
                data: &data[dptr..dptr + filesize],
                next_off: nptr,
            });
        }
        p = nptr;
    }
    None
}

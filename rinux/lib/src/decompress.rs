// SPDX-License-Identifier: GPL-2.0
//! Recognise the compression format of an initramfs or kernel image from its
//! first two bytes.
//!
//! Ported from `lib/decompress.c`: the `compressed_formats` table and
//! `decompress_method()`. The decompressors that the table points at (gunzip,
//! bunzip2, unxz, unlzma, unlzo, unlz4, unzstd) are separate kernel files and
//! are not ported here; this module names which one applies.

/// The decompressor a magic number selects. The variants follow C's table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Method {
    Gzip,
    Bzip2,
    Lzma,
    Xz,
    Lzo,
    Lz4,
    Zstd,
}

/// `compressed_formats[]`: two magic bytes, the name and the method. Gzip has
/// two magics, as in C.
const FORMATS: [([u8; 2], &str, Method); 8] = [
    ([0x1f, 0x8b], "gzip", Method::Gzip),
    ([0x1f, 0x9e], "gzip", Method::Gzip),
    ([0x42, 0x5a], "bzip2", Method::Bzip2),
    ([0x5d, 0x00], "lzma", Method::Lzma),
    ([0xfd, 0x37], "xz", Method::Xz),
    ([0x89, 0x4c], "lzo", Method::Lzo),
    ([0x02, 0x21], "lz4", Method::Lz4),
    ([0x28, 0xb5], "zstd", Method::Zstd),
];

/// `decompress_method()`: the format named by the first two bytes of `inbuf`,
/// as `(method, name)`, or `None` when the data is shorter than two bytes or
/// the magic is not known. C returns a NULL decompressor in those cases.
pub fn decompress_method(inbuf: &[u8]) -> Option<(Method, &'static str)> {
    if inbuf.len() < 2 {
        return None;
    }
    FORMATS
        .iter()
        .find(|(magic, _, _)| magic[..] == inbuf[..2])
        .map(|&(_, name, method)| (method, name))
}

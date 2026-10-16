// SPDX-License-Identifier: GPL-2.0-or-later
//! Host tests for the compression magic table. Linux has no KUnit file for it.

use rinux_lib::decompress::{Method, decompress_method};

#[test]
fn every_magic_selects_its_format() {
    let cases: [([u8; 2], &str, Method); 8] = [
        ([0x1f, 0x8b], "gzip", Method::Gzip),
        ([0x1f, 0x9e], "gzip", Method::Gzip),
        ([0x42, 0x5a], "bzip2", Method::Bzip2),
        ([0x5d, 0x00], "lzma", Method::Lzma),
        ([0xfd, 0x37], "xz", Method::Xz),
        ([0x89, 0x4c], "lzo", Method::Lzo),
        ([0x02, 0x21], "lz4", Method::Lz4),
        ([0x28, 0xb5], "zstd", Method::Zstd),
    ];
    for (magic, name, method) in cases {
        let mut data = magic.to_vec();
        data.extend_from_slice(b"rest of the stream");
        assert_eq!(decompress_method(&data), Some((method, name)));
    }
}

#[test]
fn short_or_unknown_data_selects_nothing() {
    assert_eq!(decompress_method(&[]), None);
    assert_eq!(decompress_method(&[0x1f]), None);
    assert_eq!(decompress_method(b"\x00\x00data"), None);
}

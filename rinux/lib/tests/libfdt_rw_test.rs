// SPDX-License-Identifier: GPL-2.0-or-later OR BSD-2-Clause
//! Host tests for the libfdt write side: the sequential builder, the in-place
//! editor, packing and the copy helpers. Trees are built in the test. Linux
//! has no KUnit file for these; the checks follow the C behaviour.

use rinux_lib::libfdt::{Fdt, FdtError, FdtMut, FdtSw, create_empty_tree, open_into, sw_resize};

const BEGIN: u32 = 1;
const END_NODE: u32 = 2;
const PROP: u32 = 3;
const END: u32 = 9;

fn push_u32(out: &mut Vec<u8>, v: u32) {
    out.extend_from_slice(&v.to_be_bytes());
}

fn pad4(out: &mut Vec<u8>) {
    while out.len() % 4 != 0 {
        out.push(0);
    }
}

fn begin(out: &mut Vec<u8>, name: &str) {
    push_u32(out, BEGIN);
    out.extend_from_slice(name.as_bytes());
    out.push(0);
    pad4(out);
}

fn prop(out: &mut Vec<u8>, nameoff: u32, value: &[u8]) {
    push_u32(out, PROP);
    push_u32(out, value.len() as u32);
    push_u32(out, nameoff);
    out.extend_from_slice(value);
    pad4(out);
}

/// A version 17 blob with a root holding `compatible = "foo", "bar"`, a child
/// `cpu@0` with `phandle = <1>`, an `aliases` node with `serial0`, and one
/// reservation. Built the same way as the read-only tests.
fn sample_blob() -> Vec<u8> {
    let strings = b"compatible\0phandle\0serial0\0";
    let mut structs = Vec::new();
    begin(&mut structs, "");
    prop(&mut structs, 0, b"foo\0bar\0");
    begin(&mut structs, "cpu@0");
    prop(&mut structs, 11, &1u32.to_be_bytes());
    push_u32(&mut structs, END_NODE);
    begin(&mut structs, "aliases");
    prop(&mut structs, 19, b"/cpu@0\0");
    push_u32(&mut structs, END_NODE);
    push_u32(&mut structs, END_NODE);
    push_u32(&mut structs, END);

    let mut rsv = Vec::new();
    rsv.extend_from_slice(&0x1000u64.to_be_bytes());
    rsv.extend_from_slice(&0x100u64.to_be_bytes());
    rsv.extend_from_slice(&[0u8; 16]);

    let off_mem = 40usize;
    let off_struct = off_mem + rsv.len();
    let off_strings = off_struct + structs.len();
    let total = off_strings + strings.len();

    let mut blob = Vec::new();
    push_u32(&mut blob, 0xd00d_feed);
    push_u32(&mut blob, total as u32);
    push_u32(&mut blob, off_struct as u32);
    push_u32(&mut blob, off_strings as u32);
    push_u32(&mut blob, off_mem as u32);
    push_u32(&mut blob, 17);
    push_u32(&mut blob, 16);
    push_u32(&mut blob, 0);
    push_u32(&mut blob, strings.len() as u32);
    push_u32(&mut blob, structs.len() as u32);
    blob.extend_from_slice(&rsv);
    blob.extend_from_slice(&structs);
    blob.extend_from_slice(strings);
    blob
}

/// The sample blob copied into a buffer with room to grow.
fn roomy(extra: usize) -> Vec<u8> {
    let blob = sample_blob();
    let mut buf = vec![0u8; blob.len() + extra];
    buf[..blob.len()].copy_from_slice(&blob);
    // As `fdt_open_into()` does: the header's totalsize is the buffer size,
    // so the edits have room to grow the tree.
    let len = buf.len() as u32;
    buf[4..8].copy_from_slice(&len.to_be_bytes());
    buf
}

#[test]
fn sequential_builder_makes_a_readable_tree() {
    let mut buf = vec![0u8; 256];
    let mut sw = FdtSw::create(&mut buf, 0).unwrap();
    sw.finish_reservemap().unwrap();
    sw.begin_node(b"").unwrap();
    sw.property(b"compatible", b"acme,board\0").unwrap();
    sw.begin_node(b"cpu@0").unwrap();
    sw.property(b"reg", &0x42u32.to_be_bytes()).unwrap();
    sw.end_node().unwrap();
    sw.end_node().unwrap();
    sw.finish().unwrap();

    let fdt = Fdt::new(&buf).unwrap();
    assert_eq!(Fdt::check_header(&buf), Ok(()));
    let root = fdt.path_offset(b"/").unwrap();
    assert_eq!(fdt.getprop(root, b"compatible").unwrap(), b"acme,board\0");
    let cpu = fdt.path_offset(b"/cpu@0").unwrap();
    assert_eq!(fdt.getprop(cpu, b"reg").unwrap(), &0x42u32.to_be_bytes());
}

#[test]
fn sequential_builder_checks_its_state() {
    let mut buf = vec![0u8; 256];
    let mut sw = FdtSw::create(&mut buf, 0).unwrap();
    // Nodes may not be written before the reservation map is finished.
    assert_eq!(sw.begin_node(b""), Err(FdtError::BadState));
    sw.finish_reservemap().unwrap();
    assert_eq!(sw.add_reservemap_entry(1, 1), Err(FdtError::BadState));
}

#[test]
fn create_rejects_bad_flags_and_small_buffers() {
    let mut buf = vec![0u8; 256];
    assert_eq!(
        FdtSw::create(&mut buf, 0x80).err(),
        Some(FdtError::BadFlags)
    );
    let mut small = vec![0u8; 16];
    assert_eq!(FdtSw::create(&mut small, 0).err(), Some(FdtError::NoSpace));
}

#[test]
fn open_rejects_a_blob_that_is_not_writable() {
    let mut buf = vec![0u8; 256];
    FdtSw::create(&mut buf, 0).unwrap();
    // An unfinished tree has no structure yet, so it cannot be edited.
    assert_eq!(FdtMut::open(&mut buf).err(), Some(FdtError::BadState));
}

#[test]
fn setprop_resizes_and_appends() {
    let mut buf = roomy(64);
    let mut m = FdtMut::open(&mut buf).unwrap();
    let root = m.as_fdt().path_offset(b"/").unwrap();

    m.setprop(root, b"model", b"x\0").unwrap();
    assert_eq!(m.as_fdt().getprop(root, b"model").unwrap(), b"x\0");

    m.setprop(root, b"model", b"longer value\0").unwrap();
    assert_eq!(
        m.as_fdt().getprop(root, b"model").unwrap(),
        b"longer value\0"
    );

    m.appendprop(root, b"compatible", b"baz\0").unwrap();
    let fdt = m.as_fdt();
    assert_eq!(fdt.stringlist_count(root, b"compatible"), Ok(3));
    assert_eq!(fdt.stringlist_search(root, b"compatible", b"baz"), Ok(2));
}

#[test]
fn delprop_and_nop_property_hide_a_property() {
    let mut buf = roomy(64);
    let mut m = FdtMut::open(&mut buf).unwrap();
    let root = m.as_fdt().path_offset(b"/").unwrap();
    m.setprop(root, b"a", b"1\0").unwrap();
    m.setprop(root, b"b", b"2\0").unwrap();
    m.nop_property(root, b"a").unwrap();
    assert_eq!(m.as_fdt().getprop(root, b"a"), Err(FdtError::NotFound));
    m.delprop(root, b"b").unwrap();
    assert_eq!(m.as_fdt().getprop(root, b"b"), Err(FdtError::NotFound));
    assert_eq!(
        m.as_fdt().getprop(root, b"compatible").unwrap(),
        b"foo\0bar\0"
    );
}

#[test]
fn add_and_delete_nodes_and_rename() {
    let mut buf = roomy(128);
    let mut m = FdtMut::open(&mut buf).unwrap();
    let root = m.as_fdt().path_offset(b"/").unwrap();
    let new = m.add_subnode(root, b"new").unwrap();
    assert_eq!(m.add_subnode(root, b"new"), Err(FdtError::Exists));
    assert_eq!(m.as_fdt().path_offset(b"/new"), Ok(new));
    m.set_name(new, b"renamed").unwrap();
    assert!(m.as_fdt().path_offset(b"/renamed").is_ok());
    assert_eq!(m.as_fdt().path_offset(b"/new"), Err(FdtError::NotFound));

    let cpu = m.as_fdt().path_offset(b"/cpu@0").unwrap();
    m.del_node(cpu).unwrap();
    assert_eq!(m.as_fdt().path_offset(b"/cpu@0"), Err(FdtError::NotFound));
    // The phandle lived on the deleted node.
    assert_eq!(
        m.as_fdt().node_offset_by_phandle(1),
        Err(FdtError::NotFound)
    );
}

#[test]
fn mem_rsv_add_and_delete() {
    let mut buf = roomy(64);
    let mut m = FdtMut::open(&mut buf).unwrap();
    m.add_mem_rsv(0x2000, 0x10).unwrap();
    assert_eq!(m.as_fdt().num_mem_rsv(), Ok(2));
    assert_eq!(m.as_fdt().get_mem_rsv(1), Ok((0x2000, 0x10)));
    m.del_mem_rsv(0).unwrap();
    assert_eq!(m.as_fdt().num_mem_rsv(), Ok(1));
    assert_eq!(m.as_fdt().get_mem_rsv(0), Ok((0x2000, 0x10)));
    assert_eq!(m.del_mem_rsv(5), Err(FdtError::NotFound));
}

#[test]
fn addrrange_uses_the_default_cells() {
    let mut buf = roomy(64);
    let mut m = FdtMut::open(&mut buf).unwrap();
    let root = m.as_fdt().path_offset(b"/").unwrap();
    // No #address-cells or #size-cells: 2 and 1, so 12 bytes.
    m.appendprop_addrrange(root, root, b"reg", 0x1_0000_0000, 0x100)
        .unwrap();
    let reg = m.as_fdt().getprop(root, b"reg").unwrap();
    assert_eq!(reg.len(), 12);
    assert_eq!(&reg[..8], &0x1_0000_0000u64.to_be_bytes());
    assert_eq!(&reg[8..], &0x100u32.to_be_bytes());
}

#[test]
fn pack_keeps_the_tree_readable() {
    let mut buf = roomy(256);
    let mut m = FdtMut::open(&mut buf).unwrap();
    let root = m.as_fdt().path_offset(b"/").unwrap();
    m.setprop(root, b"model", b"packed\0").unwrap();
    m.pack().unwrap();
    let fdt = Fdt::new(&buf).unwrap();
    assert_eq!(Fdt::check_header(&buf), Ok(()));
    assert_eq!(fdt.getprop(0, b"model").unwrap(), b"packed\0");
    assert!(fdt.path_offset(b"/cpu@0").is_ok());
}

#[test]
fn open_into_copies_a_tree() {
    let src = sample_blob();
    let mut dst = vec![0u8; src.len() + 32];
    open_into(&src, &mut dst).unwrap();
    let fdt = Fdt::new(&dst).unwrap();
    assert_eq!(fdt.version(), 17);
    assert_eq!(fdt.totalsize(), dst.len());
    assert!(fdt.path_offset(b"/cpu@0").is_ok());
    assert_eq!(open_into(&src, &mut dst[..8]), Err(FdtError::NoSpace));
}

#[test]
fn sw_resize_moves_an_unfinished_tree() {
    let mut small = vec![0u8; 128];
    {
        let mut sw = FdtSw::create(&mut small, 0).unwrap();
        sw.finish_reservemap().unwrap();
        sw.begin_node(b"").unwrap();
        sw.property(b"k", b"v\0").unwrap();
        sw.end_node().unwrap();
    }
    let mut big = vec![0u8; 512];
    sw_resize(&small, &mut big).unwrap();
    // The unfinished tree still reads: its strings sit at the end of the new buffer.
    let fdt = Fdt::new(&big).unwrap();
    assert_eq!(fdt.totalsize(), big.len());
    assert_eq!(fdt.getprop(0, b"k"), Ok(&b"v "[..]));
}

#[test]
fn empty_tree_has_a_root_only() {
    let mut buf = vec![0u8; 256];
    create_empty_tree(&mut buf).unwrap();
    let fdt = Fdt::new(&buf).unwrap();
    assert_eq!(fdt.path_offset(b"/"), Ok(0));
    assert_eq!(fdt.num_mem_rsv(), Ok(0));
    assert_eq!(fdt.first_subnode(0), Err(FdtError::NotFound));
    assert_eq!(fdt.getprop(0, b"compatible"), Err(FdtError::NotFound));
}

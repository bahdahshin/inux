// SPDX-License-Identifier: GPL-2.0-or-later OR BSD-2-Clause
//! Host tests for the read-only libfdt port, over a blob built in the test.
//! Linux has no KUnit file for these; the checks follow the C behaviour.

use rinux_lib::libfdt::{Fdt, FdtError, find_string_len, strerror, stringlist_contains};

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

/// Build a version 17 blob:
///
/// ```text
/// /  compatible = "foo", "bar"
/// /cpu@0  phandle = <1>
/// /aliases  serial0 = "/cpu@0"
/// ```
///
/// with one memory reservation at 0x1000, size 0x100.
fn sample_blob() -> Vec<u8> {
    let strings = b"compatible\0phandle\0serial0\0";
    let (compat_off, phandle_off, serial_off) = (0u32, 11u32, 19u32);

    let mut structs = Vec::new();
    begin(&mut structs, "");
    prop(&mut structs, compat_off, b"foo\0bar\0");
    begin(&mut structs, "cpu@0");
    prop(&mut structs, phandle_off, &1u32.to_be_bytes());
    push_u32(&mut structs, END_NODE);
    begin(&mut structs, "aliases");
    prop(&mut structs, serial_off, b"/cpu@0\0");
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
    push_u32(&mut blob, 17); // version
    push_u32(&mut blob, 16); // last compatible version
    push_u32(&mut blob, 0); // boot_cpuid_phys
    push_u32(&mut blob, strings.len() as u32);
    push_u32(&mut blob, structs.len() as u32);
    blob.extend_from_slice(&rsv);
    blob.extend_from_slice(&structs);
    blob.extend_from_slice(strings);
    assert_eq!(blob.len(), total);
    blob
}

#[test]
fn header_checks_pass_on_a_built_blob() {
    let blob = sample_blob();
    assert_eq!(Fdt::check_header(&blob), Ok(()));
    let fdt = Fdt::new(&blob).unwrap();
    assert_eq!(fdt.version(), 17);
    assert_eq!(fdt.totalsize(), blob.len());
}

#[test]
fn bad_magic_and_truncation_are_reported() {
    let mut blob = sample_blob();
    blob[0] = 0;
    assert_eq!(Fdt::new(&blob).unwrap_err(), FdtError::BadMagic);

    let blob = sample_blob();
    assert_eq!(
        Fdt::new(&blob[..blob.len() - 1]).unwrap_err(),
        FdtError::Truncated
    );
    assert_eq!(Fdt::new(&blob[..20]).unwrap_err(), FdtError::Truncated);
}

#[test]
fn bad_version_is_reported() {
    let mut blob = sample_blob();
    blob[20..24].copy_from_slice(&1u32.to_be_bytes());
    assert_eq!(Fdt::new(&blob).unwrap_err(), FdtError::BadVersion);
}

#[test]
fn node_names_and_paths() {
    let blob = sample_blob();
    let fdt = Fdt::new(&blob).unwrap();
    let root = fdt.path_offset(b"/").unwrap();
    assert_eq!(root, 0);
    let cpu = fdt.path_offset(b"/cpu@0").unwrap();
    assert_eq!(fdt.get_name(cpu).unwrap(), b"cpu@0");
    // A name without '@' matches a child whose unit address follows it.
    assert_eq!(fdt.path_offset(b"/cpu"), Ok(cpu));
    assert_eq!(fdt.path_offset(b"/missing"), Err(FdtError::NotFound));
    assert_eq!(fdt.path_offset(b""), Err(FdtError::BadPath));

    let mut buf = [0u8; 32];
    let n = fdt.get_path(cpu, &mut buf).unwrap();
    assert_eq!(&buf[..n], b"/cpu@0");
    assert_eq!(buf[n], 0);
    assert_eq!(fdt.get_path(cpu, &mut [0u8; 3]), Err(FdtError::NoSpace));
}

#[test]
fn walk_children_and_depths() {
    let blob = sample_blob();
    let fdt = Fdt::new(&blob).unwrap();
    let root = fdt.path_offset(b"/").unwrap();
    let cpu = fdt.first_subnode(root).unwrap();
    assert_eq!(fdt.get_name(cpu).unwrap(), b"cpu@0");
    let aliases = fdt.next_subnode(cpu).unwrap();
    assert_eq!(fdt.get_name(aliases).unwrap(), b"aliases");
    assert_eq!(fdt.next_subnode(aliases), Err(FdtError::NotFound));
    assert_eq!(fdt.node_depth(root), Ok(0));
    assert_eq!(fdt.node_depth(cpu), Ok(1));
    assert_eq!(fdt.parent_offset(cpu), Ok(root));
    assert_eq!(fdt.parent_offset(root), Err(FdtError::NotFound));
}

#[test]
fn properties_and_string_lists() {
    let blob = sample_blob();
    let fdt = Fdt::new(&blob).unwrap();
    let root = fdt.path_offset(b"/").unwrap();
    assert_eq!(fdt.getprop(root, b"compatible").unwrap(), b"foo\0bar\0");
    assert_eq!(fdt.getprop(root, b"missing"), Err(FdtError::NotFound));
    assert_eq!(fdt.stringlist_count(root, b"compatible"), Ok(2));
    assert_eq!(fdt.stringlist_search(root, b"compatible", b"bar"), Ok(1));
    assert_eq!(fdt.stringlist_get(root, b"compatible", 0), Ok(&b"foo"[..]));
    assert_eq!(
        fdt.stringlist_get(root, b"compatible", 2),
        Err(FdtError::NotFound)
    );
    assert_eq!(fdt.node_check_compatible(root, b"foo"), Ok(true));
    assert_eq!(fdt.node_offset_by_compatible(None, b"bar"), Ok(root));

    let cpu = fdt.path_offset(b"/cpu@0").unwrap();
    assert_eq!(fdt.get_phandle(cpu), 1);
    assert_eq!(fdt.node_offset_by_phandle(1), Ok(cpu));
    assert_eq!(fdt.node_offset_by_phandle(0), Err(FdtError::BadPhandle));
    assert_eq!(fdt.find_max_phandle(), Ok(1));
    assert_eq!(fdt.generate_phandle(), Ok(2));
    assert_eq!(
        fdt.node_offset_by_prop_value(None, b"phandle", &1u32.to_be_bytes()),
        Ok(cpu)
    );
}

#[test]
fn aliases_and_reservations() {
    let blob = sample_blob();
    let fdt = Fdt::new(&blob).unwrap();
    let cpu = fdt.path_offset(b"/cpu@0").unwrap();
    assert_eq!(fdt.get_alias(b"serial0"), Ok(&b"/cpu@0"[..]));
    assert_eq!(fdt.get_alias(b"serial1"), Err(FdtError::NotFound));
    assert_eq!(fdt.path_offset(b"serial0"), Ok(cpu));
    assert_eq!(fdt.path_offset(b"serial0/nothing"), Err(FdtError::NotFound));

    assert_eq!(fdt.num_mem_rsv(), Ok(1));
    assert_eq!(fdt.get_mem_rsv(0), Ok((0x1000, 0x100)));
    // Entry 1 is the zero terminator, which C also returns as a valid entry.
    assert_eq!(fdt.get_mem_rsv(1), Ok((0, 0)));
}

#[test]
fn move_copies_the_blob() {
    let blob = sample_blob();
    let fdt = Fdt::new(&blob).unwrap();
    let mut out = vec![0u8; blob.len()];
    fdt.move_to(&mut out).unwrap();
    assert_eq!(out, blob);
    assert_eq!(fdt.move_to(&mut out[..4]), Err(FdtError::NoSpace));
}

#[test]
fn strerror_and_helpers() {
    assert_eq!(strerror(-(FdtError::Alignment.code())), "FDT_ERR_ALIGNMENT");
    assert!(stringlist_contains(b"ab\0cd\0", b"cd"));
    assert_eq!(find_string_len(b"x\0yz\0", b"yz"), Some(2));
}

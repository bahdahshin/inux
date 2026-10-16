// SPDX-License-Identifier: GPL-2.0-or-later
//! Host tests for `lib/fw_table.c`. Linux has no KUnit file for it, so these
//! build small CDAT and ACPI-style tables and check the walk rules.

use rinux_lib::errno::{EINVAL, EOPNOTSUPP};
use rinux_lib::fw_table::{SubtableProc, acpi_parse_entries_array, cdat_table_parse};

/// Build a CDAT table: the 16-byte header with its total length, then one
/// subtable per `(type, payload)` pair.
fn cdat_table(subtables: &[(u8, &[u8])]) -> Vec<u8> {
    let mut t = vec![0u8; 16];
    for &(ty, payload) in subtables {
        let len = 4 + payload.len();
        t.push(ty);
        t.push(0);
        t.extend_from_slice(&(len as u16).to_le_bytes());
        t.extend_from_slice(payload);
    }
    let total = t.len() as u32;
    t[0..4].copy_from_slice(&total.to_le_bytes());
    t
}

#[test]
fn cdat_visits_only_matching_entries_in_order() {
    let table = cdat_table(&[(0, &[1, 2]), (1, &[9]), (0, &[3])]);
    let mut seen: Vec<Vec<u8>> = Vec::new();
    let count = cdat_table_parse(
        0,
        &mut |e| {
            seen.push(e.to_vec());
            Ok(())
        },
        &table,
        0,
    );

    assert_eq!(count, Ok(2));
    assert_eq!(seen, vec![vec![0, 0, 6, 0, 1, 2], vec![0, 0, 5, 0, 3]]);
}

#[test]
fn cdat_with_no_match_counts_zero() {
    let table = cdat_table(&[(1, &[9])]);
    let count = cdat_table_parse(7, &mut |_| Ok(()), &table, 0);
    assert_eq!(count, Ok(0));
}

#[test]
fn zero_length_entry_is_rejected() {
    // One subtable whose length field is zero. The extra byte keeps the
    // header inside the table, since the walk stops once a header no longer
    // fits before the table end.
    let mut table = vec![0u8; 16];
    table.extend_from_slice(&[0, 0, 0, 0, 0]);
    let total = table.len() as u32;
    table[0..4].copy_from_slice(&total.to_le_bytes());

    assert_eq!(cdat_table_parse(0, &mut |_| Ok(()), &table, 0), Err(EINVAL));
}

#[test]
fn handler_error_stops_the_walk_as_einval() {
    let table = cdat_table(&[(0, &[1]), (0, &[2])]);
    let mut calls = 0;
    let result = cdat_table_parse(
        0,
        &mut |_| {
            calls += 1;
            Err(EOPNOTSUPP)
        },
        &table,
        0,
    );

    assert_eq!(result, Err(EINVAL));
    assert_eq!(calls, 1);
}

#[test]
fn max_entries_caps_handler_calls_but_not_the_count() {
    let table = cdat_table(&[(0, &[1]), (0, &[2]), (0, &[3])]);
    let mut calls = 0;
    let mut procs = [SubtableProc {
        id: 0,
        count: 0,
        handler: &mut |_| {
            calls += 1;
            Ok(())
        },
    }];
    let count = acpi_parse_entries_array(b"CDAT", 16, &table, 0, &mut procs, 2);

    assert_eq!(count, Ok(3));
    assert_eq!(procs[0].count, 3);
    assert_eq!(calls, 2);
}

#[test]
fn max_length_stops_the_walk_early() {
    // Three 6-byte subtables after the 16-byte header.
    let table = cdat_table(&[(0, &[1, 1]), (0, &[2, 2]), (0, &[3, 3])]);
    // The limit ends right after the second subtable.
    let count = cdat_table_parse(0, &mut |_| Ok(()), &table, 16 + 6 + 6);
    assert_eq!(count, Ok(2));
}

#[test]
fn procs_match_their_own_types_and_ignore_the_rest() {
    let table = cdat_table(&[(0, &[]), (1, &[]), (0, &[]), (2, &[])]);
    let mut calls0 = 0;
    let mut calls1 = 0;
    let mut procs = [
        SubtableProc {
            id: 0,
            count: 0,
            handler: &mut |_| {
                calls0 += 1;
                Ok(())
            },
        },
        SubtableProc {
            id: 1,
            count: 0,
            handler: &mut |_| {
                calls1 += 1;
                Ok(())
            },
        },
    ];
    let count = acpi_parse_entries_array(b"CDAT", 16, &table, 0, &mut procs, 0);

    assert_eq!(count, Ok(3));
    assert_eq!(procs[0].count, 2);
    assert_eq!(procs[1].count, 1);
    assert_eq!(calls0, 2);
    assert_eq!(calls1, 1);
}

#[test]
fn common_layout_uses_u8_type_and_length_and_acpi_table_length() {
    // Signature, then the ACPI length at offset 4, then an 8-byte root header.
    let mut table = b"SRAT".to_vec();
    table.extend_from_slice(&[0, 0, 0, 0]);
    table.extend_from_slice(&[1, 3, 0xAA]); // type 1, length 3
    table.extend_from_slice(&[1, 2]); // type 1, header only
    table.extend_from_slice(&[2, 2]); // type 2, not requested
    let total = table.len() as u32;
    table[4..8].copy_from_slice(&total.to_le_bytes());

    let mut seen: Vec<Vec<u8>> = Vec::new();
    let mut procs = [SubtableProc {
        id: 1,
        count: 0,
        handler: &mut |e| {
            seen.push(e.to_vec());
            Ok(())
        },
    }];
    let count = acpi_parse_entries_array(b"SRAT", 8, &table, 0, &mut procs, 0);

    assert_eq!(count, Ok(2));
    assert_eq!(seen, vec![vec![1, 3, 0xAA], vec![1, 2]]);
}

#[test]
fn unported_signatures_are_not_supported() {
    let table = cdat_table(&[]);
    for sig in [b"HMAT", b"PRMT", b"CEDT"] {
        let mut procs: [SubtableProc<'_>; 0] = [];
        assert_eq!(
            acpi_parse_entries_array(sig, 16, &table, 0, &mut procs, 0),
            Err(EOPNOTSUPP),
            "{}",
            String::from_utf8_lossy(sig)
        );
    }
}

#[test]
fn table_longer_than_its_buffer_is_rejected() {
    let mut table = cdat_table(&[(0, &[1])]);
    table[0..4].copy_from_slice(&100u32.to_le_bytes());
    assert_eq!(cdat_table_parse(0, &mut |_| Ok(()), &table, 0), Err(EINVAL));
}

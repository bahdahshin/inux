// SPDX-License-Identifier: GPL-2.0-or-later
//! Host tests for the exception table: sorting, lookup hits and misses.
//! Linux has no KUnit file for `extable.c`.

use rinux_lib::extable::{ExceptionEntry, search_extable, sort_extable};

fn entry(insn: u64, fixup: u64) -> ExceptionEntry {
    ExceptionEntry { insn, fixup }
}

#[test]
fn sort_then_search_finds_every_entry() {
    let mut table = [
        entry(0x300, 0x30f),
        entry(0x100, 0x10f),
        entry(0x200, 0x20f),
    ];
    sort_extable(&mut table);
    assert_eq!(
        table.iter().map(|e| e.insn).collect::<Vec<_>>(),
        [0x100, 0x200, 0x300]
    );
    for e in table {
        assert_eq!(search_extable(&table, e.insn), Some(&e));
    }
}

#[test]
fn an_address_without_a_fixup_is_not_found() {
    let mut table = [entry(0x100, 0x10f), entry(0x200, 0x20f)];
    sort_extable(&mut table);
    assert_eq!(search_extable(&table, 0x150), None);
    assert_eq!(search_extable(&table, 0x0), None);
    assert_eq!(search_extable(&table, 0x300), None);
}

#[test]
fn an_empty_table_finds_nothing() {
    assert_eq!(search_extable(&[], 0x100), None);
}

#[test]
fn a_large_shuffled_table_comes_out_sorted() {
    let mut x: u64 = 0x9e37_79b9;
    let mut table: Vec<ExceptionEntry> = (0..1000)
        .map(|_| {
            x = x
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            entry(x >> 20, x)
        })
        .collect();
    sort_extable(&mut table);
    assert!(table.windows(2).all(|w| w[0].insn <= w[1].insn));
    let probe = table[500].insn;
    assert_eq!(search_extable(&table, probe).map(|e| e.insn), Some(probe));
}

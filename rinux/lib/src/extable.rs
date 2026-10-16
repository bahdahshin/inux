// SPDX-License-Identifier: GPL-2.0
//! The exception table: which fault addresses have a recovery (fixup) address.
//!
//! Ported from `lib/extable.c`: `sort_extable()` and `search_extable()`. The
//! kernel records, for each instruction that may fault on user memory, the
//! address to resume at. The table is sorted by instruction address at build
//! time and searched by binary search on a fault.
//!
//! C's x86 build stores these as offsets from each entry (relative extables),
//! and `ex_to_insn()` adds them back. Here the entries hold absolute addresses,
//! which is what `ex_to_insn()` returns in C; the relative encoding is a layout
//! detail for the table's build, and the kernel's linker script produces it.

/// `struct exception_table_entry`, with absolute addresses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExceptionEntry {
    /// The address of the instruction that may fault (`ex_to_insn()`).
    pub insn: u64,
    /// The address to continue at after a fault.
    pub fixup: u64,
}

/// `sort_extable()`: sort the table by instruction address, as the build does.
pub fn sort_extable(table: &mut [ExceptionEntry]) {
    heapsort(table);
}

/// `search_extable()`: the entry for the instruction at `value`, if the table
/// has one. The table must be sorted by [`sort_extable`].
pub fn search_extable(table: &[ExceptionEntry], value: u64) -> Option<&ExceptionEntry> {
    table
        .binary_search_by_key(&value, |e| e.insn)
        .ok()
        .map(|i| &table[i])
}

/// In-place heapsort by instruction address. `no_std` has no `sort`, and C's
/// `sort()` is also an in-place, non-recursive sort, so the kernel's table is
/// sorted the same way with no allocation.
fn heapsort(table: &mut [ExceptionEntry]) {
    let n = table.len();
    fn sift(table: &mut [ExceptionEntry], mut root: usize, end: usize) {
        loop {
            let mut child = 2 * root + 1;
            if child >= end {
                return;
            }
            if child + 1 < end && table[child].insn < table[child + 1].insn {
                child += 1;
            }
            if table[root].insn >= table[child].insn {
                return;
            }
            table.swap(root, child);
            root = child;
        }
    }
    for start in (0..n / 2).rev() {
        sift(table, start, n);
    }
    for end in (1..n).rev() {
        table.swap(0, end);
        sift(table, 0, end);
    }
}

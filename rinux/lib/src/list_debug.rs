// SPDX-License-Identifier: GPL-2.0
//! List consistency checks, as in `lib/list_debug.c`.
//!
//! The C version reports each violated check with `CHECK_DATA_CORRUPTION`
//! (a printk and a stack dump) and tells the caller to skip the operation.
//! Here the first violated check comes back as a [`ListCorruption`]; its
//! [`message`](ListCorruption::message) is the C text without the addresses,
//! and the caller decides what to do with it. The checks run in the same
//! order as C, so the same corruption reports the same kind.
//!
//! The poison values are the 64-bit `LIST_POISON1` and `LIST_POISON2`
//! from `include/linux/poison.h`.

use crate::list::ListHead;

/// `LIST_POISON1`: what `list_del()` writes into `next`.
pub const LIST_POISON1: *const ListHead = 0xdead_0000_0000_0100 as *const ListHead;
/// `LIST_POISON2`: what `list_del()` writes into `prev`.
pub const LIST_POISON2: *const ListHead = 0xdead_0000_0000_0122 as *const ListHead;

/// A failed consistency check, in the order `lib/list_debug.c` makes them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListCorruption {
    AddPrevNull,
    AddNextNull,
    AddNextPrevMismatch,
    AddPrevNextMismatch,
    AddDoubleAdd,
    DelNextNull,
    DelPrevNull,
    DelNextPoison,
    DelPrevPoison,
    DelPrevNextMismatch,
    DelNextPrevMismatch,
}

impl ListCorruption {
    /// The C message, without the addresses it prints.
    pub const fn message(self) -> &'static str {
        match self {
            ListCorruption::AddPrevNull => "list_add corruption. prev is NULL.",
            ListCorruption::AddNextNull => "list_add corruption. next is NULL.",
            ListCorruption::AddNextPrevMismatch => "list_add corruption. next->prev should be prev",
            ListCorruption::AddPrevNextMismatch => "list_add corruption. prev->next should be next",
            ListCorruption::AddDoubleAdd => "list_add double add",
            ListCorruption::DelNextNull => "list_del corruption, entry->next is NULL",
            ListCorruption::DelPrevNull => "list_del corruption, entry->prev is NULL",
            ListCorruption::DelNextPoison => "list_del corruption, entry->next is LIST_POISON1",
            ListCorruption::DelPrevPoison => "list_del corruption, entry->prev is LIST_POISON2",
            ListCorruption::DelPrevNextMismatch => {
                "list_del corruption. prev->next should be entry"
            }
            ListCorruption::DelNextPrevMismatch => {
                "list_del corruption. next->prev should be entry"
            }
        }
    }
}

/// `__list_add_valid_or_report()`: checks that `new` can go between `prev`
/// and `next`.
///
/// # Safety
///
/// `prev` and `next`, when non-null, must point to live list nodes, and
/// `new` must point to a live node.
pub unsafe fn list_add_valid_or_report(
    new: *const ListHead,
    prev: *const ListHead,
    next: *const ListHead,
) -> Result<(), ListCorruption> {
    if prev.is_null() {
        return Err(ListCorruption::AddPrevNull);
    }
    if next.is_null() {
        return Err(ListCorruption::AddNextNull);
    }
    // SAFETY: prev and next are non-null and, by the caller's contract, live.
    unsafe {
        if !core::ptr::eq((*next).prev, prev) {
            return Err(ListCorruption::AddNextPrevMismatch);
        }
        if !core::ptr::eq((*prev).next, next) {
            return Err(ListCorruption::AddPrevNextMismatch);
        }
    }
    if new == prev || new == next {
        return Err(ListCorruption::AddDoubleAdd);
    }
    Ok(())
}

/// `__list_del_entry_valid_or_report()`: checks that `entry` can be unlinked.
///
/// # Safety
///
/// `entry` must point to a live list node whose `prev` and `next` are either
/// null or live list nodes.
pub unsafe fn list_del_entry_valid_or_report(entry: *const ListHead) -> Result<(), ListCorruption> {
    // SAFETY: entry is live, by the caller's contract.
    let (prev, next) = unsafe {
        (
            (*entry).prev as *const ListHead,
            (*entry).next as *const ListHead,
        )
    };
    if next.is_null() {
        return Err(ListCorruption::DelNextNull);
    }
    if prev.is_null() {
        return Err(ListCorruption::DelPrevNull);
    }
    if next == LIST_POISON1 {
        return Err(ListCorruption::DelNextPoison);
    }
    if prev == LIST_POISON2 {
        return Err(ListCorruption::DelPrevPoison);
    }
    // SAFETY: prev and next are non-null and, by the caller's contract, live.
    unsafe {
        if !core::ptr::eq((*prev).next, entry) {
            return Err(ListCorruption::DelPrevNextMismatch);
        }
        if !core::ptr::eq((*next).prev, entry) {
            return Err(ListCorruption::DelNextPrevMismatch);
        }
    }
    Ok(())
}

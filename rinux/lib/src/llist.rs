// SPDX-License-Identifier: GPL-2.0
//! Lock-less NULL-terminated single-linked list.
//!
//! A port of `include/linux/llist.h` and `lib/llist.c`. Multiple producers
//! may `add` while a single consumer `del_first`s, or any number of consumers
//! `del_all`, without a lock: the head is updated with a compare-exchange
//! loop (`try_cmpxchg`) or an exchange (`xchg`). The list is intrusive --
//! each node embeds an [`LlistNode`] -- and it is a LIFO stack: `add` pushes
//! at the front.
//!
//! Not ported: the `_this`/`on_list` helpers and the typed `for_each_entry`
//! macros (callers use [`iter`] plus the offset, as with the doubly-linked
//! list).

use core::ptr;
use core::sync::atomic::{AtomicPtr, Ordering};

/// A node to embed in a structure to put it on an [`LlistHead`].
pub struct LlistNode {
    pub next: *mut LlistNode,
}

impl LlistNode {
    pub const fn new() -> LlistNode {
        LlistNode {
            next: ptr::null_mut(),
        }
    }
}

impl Default for LlistNode {
    fn default() -> Self {
        Self::new()
    }
}

/// The head of a lock-less list.
pub struct LlistHead {
    first: AtomicPtr<LlistNode>,
}

// SAFETY: all access to `first` goes through atomics, and the node pointers
// are only dereferenced by code that owns the nodes' lifetimes.
unsafe impl Sync for LlistHead {}

impl LlistHead {
    /// `init_llist_head()`: an empty list.
    pub const fn new() -> LlistHead {
        LlistHead {
            first: AtomicPtr::new(ptr::null_mut()),
        }
    }

    /// `llist_empty()`.
    pub fn is_empty(&self) -> bool {
        self.first.load(Ordering::Acquire).is_null()
    }

    /// `llist_add_batch()`: push the chain `new_first..new_last` onto the
    /// front. Returns true if the list was empty before.
    ///
    /// # Safety
    ///
    /// `new_first` and `new_last` must bound a NULL-safe chain of live nodes
    /// not already on a list.
    pub unsafe fn add_batch(&self, new_first: *mut LlistNode, new_last: *mut LlistNode) -> bool {
        let mut first = self.first.load(Ordering::Relaxed);
        loop {
            // SAFETY: new_last is a live node we are linking.
            unsafe { (*new_last).next = first };
            match self.first.compare_exchange_weak(
                first,
                new_first,
                Ordering::Release,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(f) => first = f,
            }
        }
        first.is_null()
    }

    /// `llist_add()`: push one node onto the front.
    ///
    /// # Safety
    ///
    /// As for [`add_batch`](Self::add_batch).
    pub unsafe fn add(&self, new: *mut LlistNode) -> bool {
        // SAFETY: a single node is a one-element chain.
        unsafe { self.add_batch(new, new) }
    }

    /// `llist_del_all()`: atomically take the whole list, returning its first
    /// node (or null). The caller owns the returned chain.
    pub fn del_all(&self) -> *mut LlistNode {
        self.first.swap(ptr::null_mut(), Ordering::AcqRel)
    }

    /// `llist_del_first()`: pop the first node, or null if empty.
    ///
    /// # Safety
    ///
    /// Only one consumer may call this concurrently with producers; see the
    /// locking table in `llist.h`.
    pub unsafe fn del_first(&self) -> *mut LlistNode {
        let mut entry = self.first.load(Ordering::Acquire);
        loop {
            if entry.is_null() {
                return ptr::null_mut();
            }
            // SAFETY: entry is a live node until we detach it.
            let next = unsafe { (*entry).next };
            match self
                .first
                .compare_exchange_weak(entry, next, Ordering::AcqRel, Ordering::Acquire)
            {
                Ok(_) => return entry,
                Err(e) => entry = e,
            }
        }
    }
}

impl Default for LlistHead {
    fn default() -> Self {
        Self::new()
    }
}

/// `llist_reverse_order()`: reverse a detached chain, returning the new head.
///
/// # Safety
///
/// `head` must be a NULL-terminated chain of live nodes owned by the caller.
pub unsafe fn llist_reverse_order(mut head: *mut LlistNode) -> *mut LlistNode {
    let mut new_head: *mut LlistNode = ptr::null_mut();
    while !head.is_null() {
        let tmp = head;
        // SAFETY: head is a live node in the chain.
        unsafe {
            head = (*head).next;
            (*tmp).next = new_head;
        }
        new_head = tmp;
    }
    new_head
}

/// Iterates a detached chain (`llist_for_each`), yielding each node.
///
/// # Safety
///
/// `node` must head a NULL-terminated chain of live nodes that is not
/// modified during iteration.
pub unsafe fn iter(node: *mut LlistNode) -> LlistIter {
    LlistIter { node }
}

/// Iterator returned by [`iter`].
pub struct LlistIter {
    node: *mut LlistNode,
}

impl Iterator for LlistIter {
    type Item = *mut LlistNode;
    fn next(&mut self) -> Option<*mut LlistNode> {
        if self.node.is_null() {
            return None;
        }
        let cur = self.node;
        // SAFETY: cur is a live node by the iterator's contract.
        self.node = unsafe { (*cur).next };
        Some(cur)
    }
}

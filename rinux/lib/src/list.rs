// SPDX-License-Identifier: GPL-2.0
//! Intrusive circular doubly linked lists.
//!
//! Ported from `include/linux/list.h`.
//!
//! A [`ListHead`] is embedded in a larger structure; the list is circular,
//! with a separate head node whose `next`/`prev` point at the first and
//! last entries (at itself when empty). This is the kernel's `struct
//! list_head`, so the operations are the same raw-pointer splices and are
//! `unsafe`: the caller keeps the nodes alive and un-aliased for as long as
//! they are on a list.
//!
//! `container_of` is spelled with [`core::mem::offset_of!`]: given a
//! `*mut ListHead` that is the `field` of an `Outer`, [`list_entry`]
//! recovers the `*mut Outer`.

use core::ptr;

/// `struct list_head`.
#[repr(C)]
pub struct ListHead {
    pub next: *mut ListHead,
    pub prev: *mut ListHead,
}

impl ListHead {
    /// An uninitialized head; call [`ListHead::init`] before use.
    ///
    /// `LIST_HEAD_INIT` points a head at itself, but that needs the node's
    /// own address, so a fresh node starts null and is initialized in place.
    pub const fn new() -> ListHead {
        ListHead {
            next: ptr::null_mut(),
            prev: ptr::null_mut(),
        }
    }

    /// `INIT_LIST_HEAD()`: makes an empty list that points at itself.
    ///
    /// # Safety
    ///
    /// `self` must stay pinned at this address while it is used as a list.
    pub unsafe fn init(&mut self) {
        let me: *mut ListHead = self;
        self.next = me;
        self.prev = me;
    }

    /// `list_empty()`.
    ///
    /// # Safety
    ///
    /// `self` must be an initialized head.
    pub unsafe fn is_empty(&self) -> bool {
        self.next as *const ListHead == self
    }
}

impl Default for ListHead {
    fn default() -> ListHead {
        ListHead::new()
    }
}

/// `__list_add()`: inserts `new` between `prev` and `next`.
///
/// # Safety
///
/// All three pointers must be valid, pinned list nodes, with `prev` and
/// `next` adjacent on the same list.
unsafe fn __list_add(new: *mut ListHead, prev: *mut ListHead, next: *mut ListHead) {
    // SAFETY: the caller guarantees the three nodes are valid and adjacent.
    unsafe {
        (*next).prev = new;
        (*new).next = next;
        (*new).prev = prev;
        (*prev).next = new;
    }
}

/// `list_add()`: inserts `new` just after `head` (at the front).
///
/// # Safety
///
/// `new` must be an unlinked node and `head` an initialized list node.
pub unsafe fn list_add(new: *mut ListHead, head: *mut ListHead) {
    // SAFETY: the caller guarantees both nodes are valid.
    unsafe { __list_add(new, head, (*head).next) }
}

/// `list_add_tail()`: inserts `new` just before `head` (at the back).
///
/// # Safety
///
/// As for [`list_add`].
pub unsafe fn list_add_tail(new: *mut ListHead, head: *mut ListHead) {
    // SAFETY: the caller guarantees both nodes are valid.
    unsafe { __list_add(new, (*head).prev, head) }
}

/// `__list_del()`: unlinks whatever is between `prev` and `next`.
///
/// # Safety
///
/// `prev` and `next` must be valid nodes of one list.
unsafe fn __list_del(prev: *mut ListHead, next: *mut ListHead) {
    // SAFETY: the caller guarantees both nodes are valid.
    unsafe {
        (*next).prev = prev;
        (*prev).next = next;
    }
}

/// `list_del()`: unlinks `entry` and poisons its pointers.
///
/// Unlike C, the poison is null, so a use-after-remove faults.
///
/// # Safety
///
/// `entry` must be a linked node of a list.
pub unsafe fn list_del(entry: *mut ListHead) {
    // SAFETY: the caller guarantees entry is a valid linked node.
    unsafe {
        __list_del((*entry).prev, (*entry).next);
        (*entry).next = ptr::null_mut();
        (*entry).prev = ptr::null_mut();
    }
}

/// `list_del_init()`: unlinks `entry` and reinitializes it as an empty list.
///
/// # Safety
///
/// As for [`list_del`].
pub unsafe fn list_del_init(entry: *mut ListHead) {
    // SAFETY: the caller guarantees entry is a valid linked node.
    unsafe {
        __list_del((*entry).prev, (*entry).next);
        (*entry).init();
    }
}

/// `list_entry()` / `container_of()`: the outer struct holding this node.
///
/// `offset` is `offset_of!(Outer, field)`.
///
/// # Safety
///
/// `node` must be the `field` at `offset` inside a live `Outer`.
#[inline]
pub unsafe fn list_entry<Outer>(node: *mut ListHead, offset: usize) -> *mut Outer {
    // SAFETY: the caller guarantees node points at the field of an Outer.
    unsafe { (node as *mut u8).sub(offset) as *mut Outer }
}

/// Iterates the nodes of a list from front to back (`list_for_each`).
///
/// Safe to use while removing the *current* node, like
/// `list_for_each_safe`, because the next pointer is read before the body.
///
/// # Safety
///
/// `head` must be an initialized list head that outlives the iterator, and
/// no node may be freed or moved while it is reachable from the cursor.
pub struct Iter {
    next: *mut ListHead,
    head: *mut ListHead,
}

impl Iterator for Iter {
    type Item = *mut ListHead;

    fn next(&mut self) -> Option<*mut ListHead> {
        if self.next == self.head {
            return None;
        }
        let node = self.next;
        // SAFETY: node is a live list node by the iterator's contract.
        self.next = unsafe { (*node).next };
        Some(node)
    }
}

/// `list_for_each()` as an iterator.
///
/// # Safety
///
/// `head` must be an initialized head that outlives the iterator, and the
/// list must not be structurally changed during iteration except for
/// removing the node just yielded.
pub unsafe fn iter(head: *mut ListHead) -> Iter {
    // SAFETY: the caller guarantees head is a valid initialized head.
    Iter {
        next: unsafe { (*head).next },
        head,
    }
}

// ---------------------------------------------------------------------------
// list_sort(): a bottom-up, stable merge sort for `struct list_head` lists.
// Ported from `lib/list_sort.c`.
// ---------------------------------------------------------------------------

/// `merge()`: combine two null-terminated (by `next`) sorted lists `a` and
/// `b` into one, by `cmp`. `prev` pointers are not maintained here.
///
/// # Safety
///
/// `a` and `b` must be distinct, non-null, null-terminated singly-linked
/// lists of live nodes.
unsafe fn merge(
    cmp: &mut dyn FnMut(*mut ListHead, *mut ListHead) -> bool,
    mut a: *mut ListHead,
    mut b: *mut ListHead,
) -> *mut ListHead {
    let mut head: *mut ListHead = ptr::null_mut();
    let mut tail: *mut *mut ListHead = &raw mut head;
    // SAFETY: a and b stay valid; tail always points at `head` or a node's
    // `next` field, both writable.
    unsafe {
        loop {
            // On a tie take 'a', which keeps the sort stable.
            if cmp(a, b) {
                *tail = a;
                tail = &raw mut (*a).next;
                a = (*a).next;
                if a.is_null() {
                    *tail = b;
                    break;
                }
            } else {
                *tail = b;
                tail = &raw mut (*b).next;
                b = (*b).next;
                if b.is_null() {
                    *tail = a;
                    break;
                }
            }
        }
    }
    head
}

/// `merge_final()`: like [`merge`], but for the last merge -- it rebuilds the
/// `prev` links and closes the list back into a circular doubly-linked list
/// anchored at `head`.
///
/// # Safety
///
/// As for [`merge`]; `head` must be the list's anchor.
unsafe fn merge_final(
    cmp: &mut dyn FnMut(*mut ListHead, *mut ListHead) -> bool,
    head: *mut ListHead,
    mut a: *mut ListHead,
    mut b: *mut ListHead,
) {
    let mut tail = head;
    // SAFETY: all nodes stay valid and pinned.
    unsafe {
        loop {
            if cmp(a, b) {
                (*tail).next = a;
                (*a).prev = tail;
                tail = a;
                a = (*a).next;
                if a.is_null() {
                    break;
                }
            } else {
                (*tail).next = b;
                (*b).prev = tail;
                tail = b;
                b = (*b).next;
                if b.is_null() {
                    b = a;
                    break;
                }
            }
        }
        // Link the remainder of b onto the tail (b is non-null here).
        (*tail).next = b;
        loop {
            (*b).prev = tail;
            tail = b;
            b = (*b).next;
            if b.is_null() {
                break;
            }
        }
        // Close the circle.
        (*tail).next = head;
        (*head).prev = tail;
    }
}

/// `list_sort()`: stably sort the list at `head` so that for adjacent nodes
/// `x`, `y` the comparator holds `cmp(x, y)`. The comparator returns `true`
/// when its first argument should sort before or equal to its second (the
/// `cmp(..) <= 0` convention of the C version).
///
/// # Safety
///
/// `head` must be an initialized, circular doubly-linked head whose nodes
/// stay pinned and live for the duration of the call.
pub unsafe fn list_sort<F: FnMut(*mut ListHead, *mut ListHead) -> bool>(
    head: *mut ListHead,
    mut cmp: F,
) {
    // SAFETY: the caller guarantees head and its nodes are valid.
    unsafe { list_sort_impl(head, &mut cmp) }
}

unsafe fn list_sort_impl(
    head: *mut ListHead,
    cmp: &mut dyn FnMut(*mut ListHead, *mut ListHead) -> bool,
) {
    // SAFETY: the caller's contract.
    unsafe {
        let mut list = (*head).next;
        let mut pending: *mut ListHead = ptr::null_mut();
        let mut count: usize = 0;

        if list == (*head).prev {
            return; // zero or one elements
        }
        // Convert to a null-terminated singly-linked list.
        (*(*head).prev).next = ptr::null_mut();

        loop {
            // Find the least-significant clear bit in count.
            let mut bits = count;
            let mut tail: *mut *mut ListHead = &raw mut pending;
            while bits & 1 != 0 {
                tail = &raw mut (**tail).prev;
                bits >>= 1;
            }
            // Merge the two sublists the flipped bit selects.
            if bits != 0 {
                let a = *tail;
                let b = (*a).prev;
                let merged = merge(cmp, b, a);
                (*merged).prev = (*b).prev;
                *tail = merged;
            }

            // Move one element from the input to the pending list of lists.
            (*list).prev = pending;
            pending = list;
            list = (*list).next;
            (*pending).next = ptr::null_mut();
            count += 1;
            if list.is_null() {
                break;
            }
        }

        // Merge all the pending sublists together.
        list = pending;
        pending = (*pending).prev;
        loop {
            let next = (*pending).prev;
            if next.is_null() {
                break;
            }
            list = merge(cmp, pending, list);
            pending = next;
        }
        // Final merge, rebuilding the prev links and the circular structure.
        merge_final(cmp, head, pending, list);
    }
}

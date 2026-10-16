// SPDX-License-Identifier: GPL-2.0-or-later
//! Descending-priority sorted double-linked list.
//!
//! A port of `include/linux/plist.h` and `lib/plist.c`. A plist keeps its
//! nodes in a `node_list` sorted by `prio` (smallest first -- `INT_MIN` is
//! the highest priority). A second, deduplicated `prio_list` links one node
//! per distinct priority, so an insert finds its place in O(distinct
//! priorities) by walking that list from both ends at once.
//!
//! It is intrusive (each node embeds a [`PlistNode`]) and does no locking.

use core::mem::offset_of;
use core::ptr;

use crate::list::{ListHead, list_add, list_add_tail, list_del_init, list_entry};

/// The head of a plist.
pub struct PlistHead {
    pub node_list: ListHead,
}

/// A node to embed in a structure to put it on a [`PlistHead`].
#[repr(C)]
pub struct PlistNode {
    pub prio: i32,
    pub prio_list: ListHead,
    pub node_list: ListHead,
}

const PRIO_OFF: usize = offset_of!(PlistNode, prio_list);
const NODE_OFF: usize = offset_of!(PlistNode, node_list);

impl PlistHead {
    /// `plist_head_init()`.
    ///
    /// # Safety
    ///
    /// `self` must stay pinned while used.
    pub unsafe fn init(&mut self) {
        // SAFETY: the caller keeps the head pinned.
        unsafe { self.node_list.init() };
    }

    /// `plist_head_empty()`.
    ///
    /// # Safety
    ///
    /// `self` must be an initialized head.
    pub unsafe fn is_empty(&self) -> bool {
        // SAFETY: the caller guarantees initialization.
        unsafe { self.node_list.is_empty() }
    }
}

impl PlistNode {
    /// `plist_node_init()`: set the priority and empty both links.
    ///
    /// # Safety
    ///
    /// `self` must stay pinned while on a list.
    pub unsafe fn init(&mut self, prio: i32) {
        self.prio = prio;
        // SAFETY: the caller keeps the node pinned.
        unsafe {
            self.prio_list.init();
            self.node_list.init();
        }
    }
}

/// `plist_first()`: the highest-priority (smallest-`prio`) node. The list
/// must not be empty.
///
/// # Safety
///
/// `head` must be a non-empty, initialized plist.
pub unsafe fn plist_first(head: *mut PlistHead) -> *mut PlistNode {
    // SAFETY: a non-empty node_list's next links a node.
    unsafe { list_entry::<PlistNode>((*head).node_list.next, NODE_OFF) }
}

/// `plist_add()`: insert `node` keeping `head` sorted by priority.
///
/// # Safety
///
/// `node` must be an initialized, not-yet-listed node, and `head` an
/// initialized plist; both and all their nodes must stay pinned.
pub unsafe fn plist_add(node: *mut PlistNode, head: *mut PlistHead) {
    // SAFETY: the caller's contract; all pointers stay valid and pinned.
    unsafe {
        let mut node_next: *mut ListHead = &raw mut (*head).node_list;

        if !(*head).node_list.is_empty() {
            let first = plist_first(head);
            let mut iter = first;
            let last = list_entry::<PlistNode>((*first).prio_list.prev, PRIO_OFF);
            let mut reverse_iter = last;
            let mut prev: *mut PlistNode = ptr::null_mut();

            loop {
                if (*node).prio < (*iter).prio {
                    node_next = &raw mut (*iter).node_list;
                    break;
                } else if (*node).prio >= (*reverse_iter).prio {
                    prev = reverse_iter;
                    iter = list_entry::<PlistNode>((*reverse_iter).prio_list.next, PRIO_OFF);
                    if reverse_iter != last {
                        node_next = &raw mut (*iter).node_list;
                    }
                    break;
                }
                prev = iter;
                iter = list_entry::<PlistNode>((*iter).prio_list.next, PRIO_OFF);
                reverse_iter = list_entry::<PlistNode>((*reverse_iter).prio_list.prev, PRIO_OFF);
                if iter == first {
                    break;
                }
            }

            // Only the first node of each distinct priority joins prio_list.
            if prev.is_null() || (*prev).prio != (*node).prio {
                list_add_tail(&raw mut (*node).prio_list, &raw mut (*iter).prio_list);
            }
        }

        list_add_tail(&raw mut (*node).node_list, node_next);
    }
}

/// `plist_del()`: remove `node` from `head`, handing its prio_list slot to
/// the next same-priority node if there is one.
///
/// # Safety
///
/// `node` must currently be on `head`.
pub unsafe fn plist_del(node: *mut PlistNode, head: *mut PlistHead) {
    // SAFETY: the caller's contract.
    unsafe {
        if !(*node).prio_list.is_empty() {
            // If a same-priority node follows, make it the prio_list rep.
            if (*node).node_list.next != &raw mut (*head).node_list {
                let next = list_entry::<PlistNode>((*node).node_list.next, NODE_OFF);
                if (*next).prio_list.is_empty() {
                    list_add(&raw mut (*next).prio_list, &raw mut (*node).prio_list);
                }
            }
            list_del_init(&raw mut (*node).prio_list);
        }
        list_del_init(&raw mut (*node).node_list);
    }
}

/// Iterates the nodes of `head` in priority order (`plist_for_each`).
///
/// # Safety
///
/// `head` must outlive the iterator and not be structurally modified during
/// iteration (except removing the just-yielded node).
pub unsafe fn iter(head: *mut PlistHead) -> PlistIter {
    // SAFETY: the caller's contract.
    PlistIter {
        next: unsafe { (*head).node_list.next },
        head,
    }
}

/// Iterator returned by [`iter`].
pub struct PlistIter {
    next: *mut ListHead,
    head: *mut PlistHead,
}

impl Iterator for PlistIter {
    type Item = *mut PlistNode;
    fn next(&mut self) -> Option<*mut PlistNode> {
        // SAFETY: next links a node until it reaches the head sentinel.
        unsafe {
            if self.next == &raw mut (*self.head).node_list {
                return None;
            }
            let node = list_entry::<PlistNode>(self.next, NODE_OFF);
            self.next = (*self.next).next;
            Some(node)
        }
    }
}

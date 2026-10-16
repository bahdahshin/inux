// SPDX-License-Identifier: GPL-2.0-or-later
//! A timer queue: timers ordered by expiration.
//!
//! A port of `lib/timerqueue.c` (and `include/linux/timerqueue.h`): a red-black
//! tree of nodes keyed by their `expires` time, with the earliest-expiring node
//! cached so the next timer to fire is found in O(1). Built on the intrusive
//! [`rbtree`](crate::rbtree).
//!
//! As in the kernel, this code does no locking of its own: the caller must
//! serialize access. The nodes are intrusive, so the caller owns their storage
//! and keeps each one alive and pinned while it is in the queue.

use core::ptr;

use crate::rbtree::{RbNode, RbRoot, erase, insert_color, link_node, next};

/// `struct timerqueue_node`: one timer, embedding its tree node first so a
/// `*mut RbNode` and a `*mut TimerqueueNode` share an address.
#[repr(C)]
pub struct TimerqueueNode {
    pub node: RbNode,
    pub expires: u64,
}

impl TimerqueueNode {
    pub const fn new(expires: u64) -> TimerqueueNode {
        TimerqueueNode {
            node: RbNode::new(),
            expires,
        }
    }
}

/// `struct timerqueue_head`: the tree root plus the cached earliest node.
pub struct TimerqueueHead {
    rb_root: RbRoot,
    next: *mut RbNode,
}

impl TimerqueueHead {
    pub const fn new() -> TimerqueueHead {
        TimerqueueHead {
            rb_root: RbRoot::new(),
            next: ptr::null_mut(),
        }
    }

    /// `timerqueue_add()`: insert `node`, sorted by `expires`. Returns true if
    /// it is now the earliest-expiring timer in the queue.
    ///
    /// # Safety
    ///
    /// `node` must point to a node not already in a queue, and must stay alive
    /// and pinned until it is removed.
    pub unsafe fn add(&mut self, node: *mut TimerqueueNode) -> bool {
        // SAFETY: see the function contract; the tree pointers are valid.
        unsafe {
            let expires = (*node).expires;
            let mut link: *mut *mut RbNode = &mut self.rb_root.rb_node;
            let mut parent: *mut RbNode = ptr::null_mut();
            let mut leftmost = true;
            while !(*link).is_null() {
                parent = *link;
                let p = parent as *mut TimerqueueNode;
                if expires < (*p).expires {
                    link = &mut (*parent).rb_left;
                } else {
                    link = &mut (*parent).rb_right;
                    leftmost = false;
                }
            }
            link_node(&mut (*node).node, parent, link);
            insert_color(&mut (*node).node, &mut self.rb_root);
            if leftmost {
                self.next = &mut (*node).node;
            }
            leftmost
        }
    }

    /// `timerqueue_del()`: remove `node`. Returns true if the queue is still
    /// non-empty afterwards.
    ///
    /// # Safety
    ///
    /// `node` must currently be in this queue.
    pub unsafe fn del(&mut self, node: *mut TimerqueueNode) -> bool {
        // SAFETY: see the function contract.
        unsafe {
            let rbn: *mut RbNode = &mut (*node).node;
            // If this was the cached earliest, advance the cache first.
            if self.next == rbn {
                self.next = next(rbn);
            }
            erase(rbn, &mut self.rb_root);
            (*node).node = RbNode::new();
            !self.rb_root.is_empty()
        }
    }

    /// `timerqueue_getnext()`: the earliest-expiring node, or null if empty.
    pub fn getnext(&self) -> *mut TimerqueueNode {
        self.next as *mut TimerqueueNode
    }

    /// `timerqueue_iterate_next()`: the node after `node` in expiry order, or
    /// null.
    ///
    /// # Safety
    ///
    /// `node`, if non-null, must be in a queue.
    pub unsafe fn iterate_next(node: *mut TimerqueueNode) -> *mut TimerqueueNode {
        if node.is_null() {
            return ptr::null_mut();
        }
        // SAFETY: see the function contract.
        unsafe { next(&mut (*node).node) as *mut TimerqueueNode }
    }
}

impl Default for TimerqueueHead {
    fn default() -> TimerqueueHead {
        TimerqueueHead::new()
    }
}

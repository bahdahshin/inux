// SPDX-License-Identifier: GPL-2.0
//! Interval trees: intervals kept in a red-black tree, with the largest end
//! point of each subtree stored in its root, so overlap queries skip subtrees.
//!
//! Ported from `lib/interval_tree.c`, which instantiates
//! `include/linux/interval_tree_generic.h` for `struct interval_tree_node`. The
//! algorithms are the generic header's, step for step: insert records the
//! subtree maximum on the way down, the augmented callbacks of
//! `RB_DECLARE_CALLBACKS_MAX` keep it up to date through rotations and
//! removals, and the search and iteration functions visit only subtrees whose
//! maximum can overlap the query.
//!
//! Nodes are raw pointers, as in C, and so every operation is `unsafe` with the
//! same contract: nodes stay alive and pinned while they are in the tree.
//! Where C caches the leftmost node in `rb_root_cached`, this walks to it, which
//! costs a little time and changes no result.

use core::mem::offset_of;

use crate::rbtree::{
    RbAugment, RbNode, RbRoot, erase_augmented, insert_color_augmented, link_node, rb_parent,
};

/// `struct interval_tree_node`: an interval `[start, last]` (inclusive) and
/// the largest `last` in its subtree.
#[repr(C)]

pub struct IntervalNode {
    /// The red-black tree link.
    pub rb: RbNode,
    /// First point of the interval.
    pub start: u64,
    /// Last point of the interval, inclusive.
    pub last: u64,
    /// The largest `last` in the subtree rooted here (`__subtree_last`).
    subtree_last: u64,
}

impl IntervalNode {
    /// An unlinked node for `[start, last]`.
    pub const fn new(start: u64, last: u64) -> Self {
        Self {
            rb: RbNode::new(),
            start,
            last,
            subtree_last: last,
        }
    }
}

/// `rb_entry()`: the interval node that contains `rb`.
///
/// # Safety
///
/// `rb` must be the `rb` field of a live [`IntervalNode`].
unsafe fn entry(rb: *mut RbNode) -> *mut IntervalNode {
    // SAFETY: the caller guarantees `rb` is embedded in an IntervalNode.
    unsafe { (rb as *mut u8).sub(offset_of!(IntervalNode, rb)) as *mut IntervalNode }
}

/// `compute_subtree_last()`: the maximum of a node's own `last` and its
/// children's subtree maxima.
///
/// # Safety
///
/// `node` must be a live node of a tree.
unsafe fn compute_subtree_last(node: *mut IntervalNode) -> u64 {
    // SAFETY: children of a live node are live nodes or null.
    unsafe {
        let mut max = (*node).last;
        let left = (*node).rb.rb_left;
        if !left.is_null() {
            max = max.max((*entry(left)).subtree_last);
        }
        let right = (*node).rb.rb_right;
        if !right.is_null() {
            max = max.max((*entry(right)).subtree_last);
        }
        max
    }
}

/// `RB_DECLARE_CALLBACKS_MAX()` for `interval_tree_node`.
struct IntervalAugment;

impl RbAugment for IntervalAugment {
    unsafe fn propagate(&self, mut rb: *mut RbNode, stop: *mut RbNode) {
        // SAFETY: rb and its ancestors are nodes of the tree.
        unsafe {
            while !rb.is_null() && rb != stop {
                let node = entry(rb);
                let max = compute_subtree_last(node);
                if (*node).subtree_last == max {
                    // Nothing above changes either once this node is unchanged.
                    break;
                }
                (*node).subtree_last = max;
                rb = rb_parent(rb);
            }
        }
    }

    unsafe fn copy(&self, old: *mut RbNode, new: *mut RbNode) {
        // SAFETY: both are nodes of the tree.
        unsafe { (*entry(new)).subtree_last = (*entry(old)).subtree_last };
    }

    unsafe fn rotate(&self, old: *mut RbNode, new: *mut RbNode) {
        // SAFETY: both are nodes of the tree; `new` is now above `old`.
        unsafe {
            (*entry(new)).subtree_last = (*entry(old)).subtree_last;
            (*entry(old)).subtree_last = compute_subtree_last(entry(old));
        }
    }
}

/// `interval_tree_insert()`.
///
/// # Safety
///
/// `node` must not be in a tree, and `root` must be a valid tree. The node
/// stays pinned while it is in the tree.
pub unsafe fn interval_tree_insert(node: *mut IntervalNode, root: *mut RbRoot) {
    // SAFETY: the tree and the node are valid per the contract.
    unsafe {
        let mut link: *mut *mut RbNode = &mut (*root).rb_node;
        let mut parent: *mut RbNode = core::ptr::null_mut();
        let last = (*node).last;
        while !(*link).is_null() {
            parent = *link;
            let p = entry(parent);
            if (*p).subtree_last < last {
                (*p).subtree_last = last;
            }
            link = if (*node).start < (*p).start {
                &mut (*parent).rb_left
            } else {
                &mut (*parent).rb_right
            };
        }
        (*node).subtree_last = last;
        link_node(&mut (*node).rb, parent, link);
        insert_color_augmented(&mut (*node).rb, root, &IntervalAugment);
    }
}

/// `interval_tree_remove()`.
///
/// # Safety
///
/// `node` must be in the tree `root`.
pub unsafe fn interval_tree_remove(node: *mut IntervalNode, root: *mut RbRoot) {
    // SAFETY: the node is in the tree per the contract.
    unsafe { erase_augmented(&mut (*node).rb, root, &IntervalAugment) }
}

/// `interval_tree_subtree_search()`: the leftmost node under `node` that
/// overlaps `[start, last]`, or null.
///
/// # Safety
///
/// `node` must be a node of a valid tree.
pub unsafe fn interval_tree_subtree_search(
    mut node: *mut IntervalNode,
    start: u64,
    last: u64,
) -> *mut IntervalNode {
    // SAFETY: the tree is valid, so every child pointer is a live node.
    unsafe {
        loop {
            // Loop invariant: start <= node.subtree_last.
            let left = (*node).rb.rb_left;
            if !left.is_null() {
                let left_node = entry(left);
                if start <= (*left_node).subtree_last {
                    // Some node in the left subtree overlaps in its end point:
                    // the leftmost such node is the candidate.
                    node = left_node;
                    continue;
                }
            }
            if (*node).start <= last {
                // Cond1 holds here.
                if start <= (*node).last {
                    return node; // Cond2: node is the leftmost match.
                }
                node = entry((*node).rb.rb_right);
                continue;
            }
            return core::ptr::null_mut(); // No match.
        }
    }
}

/// `interval_tree_iter_first()`: the leftmost node overlapping
/// `[start, last]`, or null.
///
/// # Safety
///
/// `root` must be a valid tree.
pub unsafe fn interval_tree_iter_first(
    root: *mut RbRoot,
    start: u64,
    last: u64,
) -> *mut IntervalNode {
    // SAFETY: the tree is valid per the contract.
    unsafe {
        let top = (*root).rb_node;
        if top.is_null() {
            return core::ptr::null_mut();
        }
        let node = entry(top);
        // The root holds the largest `last` in the whole tree, so this rules
        // out a non-overlapping query in O(1).
        if (*node).subtree_last < start {
            return core::ptr::null_mut();
        }
        // Walk to the leftmost node: the tree's smallest `start`.
        let mut leftmost = top;
        while !(*leftmost).rb_left.is_null() {
            leftmost = (*leftmost).rb_left;
        }
        if (*entry(leftmost)).start > last {
            return core::ptr::null_mut();
        }
        interval_tree_subtree_search(node, start, last)
    }
}

/// `interval_tree_iter_next()`: the next node after `node` in the tree order
/// that overlaps `[start, last]`, or null.
///
/// # Safety
///
/// `node` must be a node of a valid tree.
pub unsafe fn interval_tree_iter_next(
    mut node: *mut IntervalNode,
    start: u64,
    last: u64,
) -> *mut IntervalNode {
    // SAFETY: the tree is valid per the contract.
    unsafe {
        let mut rb = (*node).rb.rb_right;
        loop {
            // Search the right subtree if it can hold a match.
            if !rb.is_null() {
                let right = entry(rb);
                if start <= (*right).subtree_last {
                    return interval_tree_subtree_search(right, start, last);
                }
            }
            // Climb until we come up from a left child.
            let mut prev;
            loop {
                let parent = rb_parent(&mut (*node).rb);
                if parent.is_null() {
                    return core::ptr::null_mut();
                }
                prev = &mut (*node).rb as *mut RbNode;
                node = entry(parent);
                rb = (*node).rb.rb_right;
                if prev != rb {
                    break;
                }
            }
            // Check whether this node overlaps.
            if last < (*node).start {
                return core::ptr::null_mut(); // !Cond1
            } else if start <= (*node).last {
                return node; // Cond2
            }
        }
    }
}

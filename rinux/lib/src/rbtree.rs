// SPDX-License-Identifier: GPL-2.0-or-later
//! Intrusive red-black trees.
//!
//! Ported from `lib/rbtree.c`, `include/linux/rbtree.h` and the
//! colour/pointer helpers of `include/linux/rbtree_augmented.h`.
//!
//! Based on "Introduction to Algorithms" by Cormen, Leiserson, Rivest and
//! Stein, as the kernel's is. Like `struct rb_node`, a [`RbNode`] is
//! embedded in a larger structure and stores its parent pointer and colour
//! packed into one word (the low bit is the colour, so nodes must be at
//! least 2-byte aligned). The augmented-tree callbacks are in [`RbAugment`];
//! the plain [`insert_color`] and [`erase`] use [`NoAugment`], as the kernel does
//! in for `rb_insert_color()`/`rb_erase()`.
//!
//! These are raw-pointer operations and so `unsafe`: the caller keeps the
//! nodes alive and pinned while they are in a tree, and keeps the tree
//! ordered (the comparison is the caller's, as in C, where the search is
//! open-coded at each call site).

use core::ptr;

const RB_RED: usize = 0;
const RB_BLACK: usize = 1;

/// The augmentation hooks of `struct rb_augment_callbacks`. Trees that keep
/// data derived from their subtrees (such as an interval tree's maximum)
/// implement these to update it on each rotation and removal.
pub trait RbAugment {
    /// `propagate()`: update the augmented data from `node` up to `stop`.
    ///
    /// # Safety
    ///
    /// `node` and `stop` must be nodes of the tree (or null).
    unsafe fn propagate(&self, node: *mut RbNode, stop: *mut RbNode);
    /// `copy()`: give `new` the augmented data of `old`, which it replaces.
    ///
    /// # Safety
    ///
    /// Both nodes must be valid nodes of the tree.
    unsafe fn copy(&self, old: *mut RbNode, new: *mut RbNode);
    /// `rotate()`: called after a rotation that made `new` the parent of `old`.
    ///
    /// # Safety
    ///
    /// Both nodes must be valid nodes of the tree.
    unsafe fn rotate(&self, old: *mut RbNode, new: *mut RbNode);
}

/// The no-op augmentation, which the plain [`insert_color`] and [`erase`] use.
pub struct NoAugment;

impl RbAugment for NoAugment {
    unsafe fn propagate(&self, _node: *mut RbNode, _stop: *mut RbNode) {}
    unsafe fn copy(&self, _old: *mut RbNode, _new: *mut RbNode) {}
    unsafe fn rotate(&self, _old: *mut RbNode, _new: *mut RbNode) {}
}

/// `struct rb_node`.
#[repr(C, align(8))]
pub struct RbNode {
    /// Parent pointer with the colour in bit 0 (`__rb_parent_color`).
    parent_color: usize,
    pub rb_right: *mut RbNode,
    pub rb_left: *mut RbNode,
}

impl RbNode {
    pub const fn new() -> RbNode {
        RbNode {
            parent_color: 0,
            rb_right: ptr::null_mut(),
            rb_left: ptr::null_mut(),
        }
    }
}

impl Default for RbNode {
    fn default() -> RbNode {
        RbNode::new()
    }
}

/// `struct rb_root`.
#[repr(C)]
pub struct RbRoot {
    pub rb_node: *mut RbNode,
}

impl RbRoot {
    pub const fn new() -> RbRoot {
        RbRoot {
            rb_node: ptr::null_mut(),
        }
    }

    /// Whether the tree is empty.
    pub fn is_empty(&self) -> bool {
        self.rb_node.is_null()
    }
}

impl Default for RbRoot {
    fn default() -> RbRoot {
        RbRoot::new()
    }
}

#[inline]
/// `rb_parent()`: the parent of `n`, or null for the root.
///
/// # Safety
///
/// `n` must be a valid node.
pub unsafe fn rb_parent(n: *mut RbNode) -> *mut RbNode {
    // SAFETY: n is a valid node.
    (unsafe { (*n).parent_color } & !3) as *mut RbNode
}

/// `rb_red_parent()`: the parent of a red node (colour bit known 0).
#[inline]
unsafe fn rb_red_parent(n: *mut RbNode) -> *mut RbNode {
    // SAFETY: n is a valid node.
    unsafe { (*n).parent_color as *mut RbNode }
}

#[inline]
unsafe fn rb_color(n: *mut RbNode) -> usize {
    // SAFETY: n is a valid node.
    unsafe { (*n).parent_color & 1 }
}

#[inline]
unsafe fn rb_is_red(n: *mut RbNode) -> bool {
    // SAFETY: n is a valid node.
    unsafe { rb_color(n) == RB_RED }
}

#[inline]
unsafe fn rb_is_black(n: *mut RbNode) -> bool {
    // SAFETY: n is a valid node.
    unsafe { rb_color(n) == RB_BLACK }
}

#[inline]
unsafe fn rb_set_parent(n: *mut RbNode, p: *mut RbNode) {
    // SAFETY: n is a valid node.
    unsafe { (*n).parent_color = rb_color(n) + p as usize }
}

#[inline]
unsafe fn rb_set_parent_color(n: *mut RbNode, p: *mut RbNode, color: usize) {
    // SAFETY: n is a valid node.
    unsafe { (*n).parent_color = p as usize + color }
}

#[inline]
unsafe fn rb_set_black(n: *mut RbNode) {
    // SAFETY: n is a valid node.
    unsafe { (*n).parent_color |= RB_BLACK }
}

/// `__rb_change_child()`.
#[inline]
unsafe fn change_child(old: *mut RbNode, new: *mut RbNode, parent: *mut RbNode, root: *mut RbRoot) {
    // SAFETY: the caller guarantees the nodes and root are valid.
    unsafe {
        if !parent.is_null() {
            if (*parent).rb_left == old {
                (*parent).rb_left = new;
            } else {
                (*parent).rb_right = new;
            }
        } else {
            (*root).rb_node = new;
        }
    }
}

/// `__rb_rotate_set_parents()`.
#[inline]
unsafe fn rotate_set_parents<A: RbAugment>(
    old: *mut RbNode,
    new: *mut RbNode,
    root: *mut RbRoot,
    color: usize,
    aug: &A,
) {
    // SAFETY: the caller guarantees the nodes and root are valid.
    unsafe {
        let parent = rb_parent(old);
        (*new).parent_color = (*old).parent_color;
        rb_set_parent_color(old, new, color);
        change_child(old, new, parent, root);
        aug.rotate(old, new);
    }
}

/// `rb_link_node()`: attaches `node` below `parent` at `rb_link`, ready for
/// [`insert_color`]. `rb_link` is `&parent->rb_left` or `&parent->rb_right`
/// (or `&root->rb_node` for the first node).
///
/// # Safety
///
/// `rb_link` must be the child slot found by an ordered search for `node`.
pub unsafe fn link_node(node: *mut RbNode, parent: *mut RbNode, rb_link: *mut *mut RbNode) {
    // SAFETY: the caller guarantees node, parent and rb_link are valid.
    unsafe {
        (*node).parent_color = parent as usize;
        (*node).rb_left = ptr::null_mut();
        (*node).rb_right = ptr::null_mut();
        *rb_link = node;
    }
}

/// `__rb_insert()` with the dummy augmentation: rebalances after a node has
/// been linked in red.
///
/// # Safety
///
/// `node` must have just been linked with [`link_node`] into `root`.
pub unsafe fn insert_color(node: *mut RbNode, root: *mut RbRoot) {
    // SAFETY: forwarded to the caller's contract.
    unsafe { insert_color_augmented(node, root, &NoAugment) }
}

/// `__rb_insert_augmented()`: as [`insert_color`], calling `aug.rotate` for each
/// rotation so the caller can keep augmented data up to date.
///
/// # Safety
///
/// As for [`insert_color`]; `aug`'s callbacks must be valid for the tree.
pub unsafe fn insert_color_augmented<A: RbAugment>(
    mut node: *mut RbNode,
    root: *mut RbRoot,
    aug: &A,
) {
    // SAFETY: all nodes reached from a valid tree are valid; preserved
    // throughout by the red-black invariants.
    unsafe {
        let mut parent = rb_red_parent(node);
        loop {
            if parent.is_null() {
                rb_set_parent_color(node, ptr::null_mut(), RB_BLACK);
                break;
            }
            if rb_is_black(parent) {
                break;
            }
            let gparent = rb_red_parent(parent);
            let mut tmp = (*gparent).rb_right;
            if parent != tmp {
                // parent == gparent->rb_left
                if !tmp.is_null() && rb_is_red(tmp) {
                    // Case 1 - colour flips.
                    rb_set_parent_color(tmp, gparent, RB_BLACK);
                    rb_set_parent_color(parent, gparent, RB_BLACK);
                    node = gparent;
                    parent = rb_parent(node);
                    rb_set_parent_color(node, parent, RB_RED);
                    continue;
                }
                tmp = (*parent).rb_right;
                if node == tmp {
                    // Case 2 - left rotate at parent.
                    tmp = (*node).rb_left;
                    (*parent).rb_right = tmp;
                    (*node).rb_left = parent;
                    if !tmp.is_null() {
                        rb_set_parent_color(tmp, parent, RB_BLACK);
                    }
                    rb_set_parent_color(parent, node, RB_RED);
                    aug.rotate(parent, node);
                    parent = node;
                    tmp = (*node).rb_right;
                }
                // Case 3 - right rotate at gparent.
                (*gparent).rb_left = tmp; // == parent->rb_right
                (*parent).rb_right = gparent;
                if !tmp.is_null() {
                    rb_set_parent_color(tmp, gparent, RB_BLACK);
                }
                rotate_set_parents(gparent, parent, root, RB_RED, aug);
                break;
            } else {
                tmp = (*gparent).rb_left;
                if !tmp.is_null() && rb_is_red(tmp) {
                    // Case 1 - colour flips.
                    rb_set_parent_color(tmp, gparent, RB_BLACK);
                    rb_set_parent_color(parent, gparent, RB_BLACK);
                    node = gparent;
                    parent = rb_parent(node);
                    rb_set_parent_color(node, parent, RB_RED);
                    continue;
                }
                tmp = (*parent).rb_left;
                if node == tmp {
                    // Case 2 - right rotate at parent.
                    tmp = (*node).rb_right;
                    (*parent).rb_left = tmp;
                    (*node).rb_right = parent;
                    if !tmp.is_null() {
                        rb_set_parent_color(tmp, parent, RB_BLACK);
                    }
                    rb_set_parent_color(parent, node, RB_RED);
                    aug.rotate(parent, node);
                    parent = node;
                    tmp = (*node).rb_left;
                }
                // Case 3 - left rotate at gparent.
                (*gparent).rb_right = tmp; // == parent->rb_left
                (*parent).rb_left = gparent;
                if !tmp.is_null() {
                    rb_set_parent_color(tmp, gparent, RB_BLACK);
                }
                rotate_set_parents(gparent, parent, root, RB_RED, aug);
                break;
            }
        }
    }
}

/// `____rb_erase_color()`: rebalances after a black node was removed.
///
/// # Safety
///
/// `parent` must be the node returned by [`erase`]'s internal unlink step.
unsafe fn erase_color<A: RbAugment>(mut parent: *mut RbNode, root: *mut RbRoot, aug: &A) {
    // SAFETY: nodes reached from a valid tree are valid.
    unsafe {
        let mut node: *mut RbNode = ptr::null_mut();
        loop {
            let mut sibling = (*parent).rb_right;
            if node != sibling {
                // node == parent->rb_left
                if rb_is_red(sibling) {
                    // Case 1 - left rotate at parent.
                    let tmp1 = (*sibling).rb_left;
                    (*parent).rb_right = tmp1;
                    (*sibling).rb_left = parent;
                    rb_set_parent_color(tmp1, parent, RB_BLACK);
                    rotate_set_parents(parent, sibling, root, RB_RED, aug);
                    sibling = tmp1;
                }
                let mut tmp1 = (*sibling).rb_right;
                if tmp1.is_null() || rb_is_black(tmp1) {
                    let tmp2 = (*sibling).rb_left;
                    if tmp2.is_null() || rb_is_black(tmp2) {
                        // Case 2 - sibling colour flip.
                        rb_set_parent_color(sibling, parent, RB_RED);
                        if rb_is_red(parent) {
                            rb_set_black(parent);
                        } else {
                            node = parent;
                            parent = rb_parent(node);
                            if !parent.is_null() {
                                continue;
                            }
                        }
                        break;
                    }
                    // Case 3 - right rotate at sibling.
                    let tmp1b = (*tmp2).rb_right;
                    (*sibling).rb_left = tmp1b;
                    (*tmp2).rb_right = sibling;
                    (*parent).rb_right = tmp2;
                    if !tmp1b.is_null() {
                        rb_set_parent_color(tmp1b, sibling, RB_BLACK);
                    }
                    aug.rotate(sibling, tmp2);
                    tmp1 = sibling;
                    sibling = tmp2;
                }
                // Case 4 - left rotate at parent + colour flips.
                let tmp2 = (*sibling).rb_left;
                (*parent).rb_right = tmp2;
                (*sibling).rb_left = parent;
                rb_set_parent_color(tmp1, sibling, RB_BLACK);
                if !tmp2.is_null() {
                    rb_set_parent(tmp2, parent);
                }
                rotate_set_parents(parent, sibling, root, RB_BLACK, aug);
                break;
            } else {
                sibling = (*parent).rb_left;
                if rb_is_red(sibling) {
                    // Case 1 - right rotate at parent.
                    let tmp1 = (*sibling).rb_right;
                    (*parent).rb_left = tmp1;
                    (*sibling).rb_right = parent;
                    rb_set_parent_color(tmp1, parent, RB_BLACK);
                    rotate_set_parents(parent, sibling, root, RB_RED, aug);
                    sibling = tmp1;
                }
                let mut tmp1 = (*sibling).rb_left;
                if tmp1.is_null() || rb_is_black(tmp1) {
                    let tmp2 = (*sibling).rb_right;
                    if tmp2.is_null() || rb_is_black(tmp2) {
                        // Case 2 - sibling colour flip.
                        rb_set_parent_color(sibling, parent, RB_RED);
                        if rb_is_red(parent) {
                            rb_set_black(parent);
                        } else {
                            node = parent;
                            parent = rb_parent(node);
                            if !parent.is_null() {
                                continue;
                            }
                        }
                        break;
                    }
                    // Case 3 - left rotate at sibling.
                    let tmp1b = (*tmp2).rb_left;
                    (*sibling).rb_right = tmp1b;
                    (*tmp2).rb_left = sibling;
                    (*parent).rb_left = tmp2;
                    if !tmp1b.is_null() {
                        rb_set_parent_color(tmp1b, sibling, RB_BLACK);
                    }
                    aug.rotate(sibling, tmp2);
                    tmp1 = sibling;
                    sibling = tmp2;
                }
                // Case 4 - right rotate at parent + colour flips.
                let tmp2 = (*sibling).rb_right;
                (*parent).rb_left = tmp2;
                (*sibling).rb_right = parent;
                rb_set_parent_color(tmp1, sibling, RB_BLACK);
                if !tmp2.is_null() {
                    rb_set_parent(tmp2, parent);
                }
                rotate_set_parents(parent, sibling, root, RB_BLACK, aug);
                break;
            }
        }
    }
}

/// `rb_erase()`: removes `node` from the tree and rebalances.
///
/// # Safety
///
/// `node` must currently be in `root`.
pub unsafe fn erase(node: *mut RbNode, root: *mut RbRoot) {
    // SAFETY: forwarded to the caller's contract.
    unsafe { erase_augmented(node, root, &NoAugment) }
}

/// `__rb_erase_augmented()`: as [`erase`], calling `aug.copy` and `aug.propagate`
/// where C updates augmented data.
///
/// # Safety
///
/// As for [`erase`]; `aug`'s callbacks must be valid for the tree.
pub unsafe fn erase_augmented<A: RbAugment>(node: *mut RbNode, root: *mut RbRoot, aug: &A) {
    // SAFETY: nodes reached from a valid tree are valid.
    unsafe {
        // __rb_erase_augmented() with dummy callbacks.
        let child = (*node).rb_right;
        let mut tmp = (*node).rb_left;
        let parent;
        let rebalance;
        let pc;
        let target;
        let mut leftmost = false;

        if tmp.is_null() {
            // Case 1: at most one child.
            pc = (*node).parent_color;
            parent = (pc & !3) as *mut RbNode;
            change_child(node, child, parent, root);
            target = parent;
            if !child.is_null() {
                (*child).parent_color = pc;
                rebalance = ptr::null_mut();
            } else {
                rebalance = if pc & 1 == RB_BLACK {
                    parent
                } else {
                    ptr::null_mut()
                };
            }
        } else if child.is_null() {
            // Still case 1, with the left child.
            pc = (*node).parent_color;
            (*tmp).parent_color = pc;
            parent = (pc & !3) as *mut RbNode;
            change_child(node, tmp, parent, root);
            target = parent;
            rebalance = ptr::null_mut();
        } else {
            let mut successor = child;
            let child2;
            tmp = (*child).rb_left;
            if tmp.is_null() {
                // Case 2: successor is the right child.
                parent = successor;
                child2 = (*successor).rb_right;
            } else {
                // Case 3: successor is leftmost under the right subtree.
                let mut s = successor;
                let mut p;
                loop {
                    p = s;
                    s = tmp;
                    tmp = (*tmp).rb_left;
                    if tmp.is_null() {
                        break;
                    }
                }
                successor = s;
                parent = p;
                child2 = (*successor).rb_right;
                (*parent).rb_left = child2;
                (*successor).rb_right = child;
                rb_set_parent(child, successor);
                leftmost = true;
            }

            aug.copy(node, successor);
            if leftmost {
                aug.propagate(parent, successor);
            }
            tmp = (*node).rb_left;
            (*successor).rb_left = tmp;
            rb_set_parent(tmp, successor);

            let pc2 = (*node).parent_color;
            tmp = (pc2 & !3) as *mut RbNode;
            change_child(node, successor, tmp, root);

            if !child2.is_null() {
                rb_set_parent_color(child2, parent, RB_BLACK);
                rebalance = ptr::null_mut();
            } else {
                rebalance = if rb_is_black(successor) {
                    parent
                } else {
                    ptr::null_mut()
                };
            }
            (*successor).parent_color = pc2;
            target = successor;
        }

        aug.propagate(target, ptr::null_mut());
        if !rebalance.is_null() {
            erase_color(rebalance, root, aug);
        }
    }
}

/// `rb_first()`: the leftmost (smallest) node.
///
/// # Safety
///
/// `root` must be a valid tree.
pub unsafe fn first(root: *mut RbRoot) -> *mut RbNode {
    // SAFETY: nodes reached from a valid tree are valid.
    unsafe {
        let mut n = (*root).rb_node;
        if n.is_null() {
            return ptr::null_mut();
        }
        while !(*n).rb_left.is_null() {
            n = (*n).rb_left;
        }
        n
    }
}

/// `rb_next()`: the in-order successor of `node`.
///
/// # Safety
///
/// `node` must be a node of a valid tree.
pub unsafe fn next(mut node: *mut RbNode) -> *mut RbNode {
    // SAFETY: nodes reached from a valid tree are valid.
    unsafe {
        if !(*node).rb_right.is_null() {
            node = (*node).rb_right;
            while !(*node).rb_left.is_null() {
                node = (*node).rb_left;
            }
            return node;
        }
        let mut parent = rb_parent(node);
        while !parent.is_null() && node == (*parent).rb_right {
            node = parent;
            parent = rb_parent(node);
        }
        parent
    }
}

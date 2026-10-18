// SPDX-License-Identifier: GPL-2.0-or-later
//! Tests for augmented red-black trees. Each node keeps the maximum key in its
//! subtree, updated through `RbAugment` as an interval tree's `__subtree_last`
//! is. After every insert and erase the test checks the augmented values
//! against the subtrees, and the red-black invariants via the in-order walk.

use rinux_lib::random32::RndState;
use rinux_lib::rbtree::*;

#[repr(C)]
struct Node {
    rb: RbNode,
    key: u32,
    max: u32,
}

const OFFSET: usize = core::mem::offset_of!(Node, rb);

unsafe fn node_of(rb: *mut RbNode) -> *mut Node {
    // SAFETY: rb is the rb field of a live Node.
    unsafe { (rb as *mut u8).sub(OFFSET) as *mut Node }
}

/// `__subtree_last`-style recomputation from a node and its children.
unsafe fn recompute(n: *mut Node) {
    // SAFETY: n is a live node of the tree.
    unsafe {
        let mut m = (*n).key;
        for child in [(*n).rb.rb_left, (*n).rb.rb_right] {
            if !child.is_null() {
                m = m.max((*node_of(child)).max);
            }
        }
        (*n).max = m;
    }
}

struct MaxAugment;

impl RbAugment for MaxAugment {
    unsafe fn propagate(&self, node: *mut RbNode, stop: *mut RbNode) {
        let mut n = node;
        while !n.is_null() && n != stop {
            // SAFETY: n is a node of the tree.
            unsafe {
                recompute(node_of(n));
                n = rb_parent(n);
            }
        }
    }

    unsafe fn copy(&self, old: *mut RbNode, new: *mut RbNode) {
        // SAFETY: both are nodes of the tree.
        unsafe { (*node_of(new)).max = (*node_of(old)).max };
    }

    unsafe fn rotate(&self, old: *mut RbNode, new: *mut RbNode) {
        // SAFETY: both are nodes of the tree, `new` is now above `old`.
        unsafe {
            (*node_of(new)).max = (*node_of(old)).max;
            recompute(node_of(old));
        }
    }
}

/// Ordered insert with the augmented update, as `rb_add_augmented()`.
unsafe fn insert(node: *mut Node, root: *mut RbRoot) {
    // SAFETY: the tree and all nodes are valid and pinned.
    unsafe {
        (*node).max = (*node).key;
        let mut link: *mut *mut RbNode = &mut (*root).rb_node;
        let mut parent: *mut RbNode = core::ptr::null_mut();
        while !(*link).is_null() {
            parent = *link;
            let p = node_of(parent);
            link = if (*node).key < (*p).key {
                &mut (*parent).rb_left
            } else {
                &mut (*parent).rb_right
            };
        }
        link_node(&mut (*node).rb, parent, link);
        MaxAugment.propagate(parent, core::ptr::null_mut());
        insert_color_augmented(&mut (*node).rb, root, &MaxAugment);
    }
}

/// Visit every node in order, checking each `max` against its subtree.
unsafe fn check(root: *mut RbRoot, expected: &[u32]) {
    // SAFETY: the tree is valid.
    unsafe {
        let mut keys = Vec::new();
        let mut n = first(root);
        while !n.is_null() {
            keys.push((*node_of(n)).key);
            n = next(n);
        }
        assert_eq!(keys, expected, "in-order keys");
        fn subtree_max(n: *mut RbNode) -> Option<u32> {
            if n.is_null() {
                return None;
            }
            // SAFETY: n is a node of the tree.
            unsafe {
                let node = node_of(n);
                let mut m = (*node).key;
                for c in [(*n).rb_left, (*n).rb_right] {
                    if let Some(cm) = subtree_max(c) {
                        m = m.max(cm);
                    }
                }
                assert_eq!((*node).max, m, "augmented max of key {}", (*node).key);
                Some(m)
            }
        }
        subtree_max((*root).rb_node);
    }
}

#[test]
#[allow(clippy::needless_range_loop)]
fn augmented_maxima_survive_random_inserts_and_erases() {
    let mut rnd = RndState::new(1);
    for round in 0..20 {
        let mut nodes: Vec<Node> = (0..64)
            .map(|_| Node {
                rb: RbNode::new(),
                key: rnd.next_u32() % 1000,
                max: 0,
            })
            .collect();
        let mut root = RbRoot::new();
        let root_ptr: *mut RbRoot = &mut root;
        let mut keys: Vec<u32> = Vec::new();
        unsafe {
            for i in 0..nodes.len() {
                let p: *mut Node = &mut nodes[i];
                insert(p, root_ptr);
                keys.push((*p).key);
                keys.sort_unstable();
                check(root_ptr, &keys);
            }
            // Erase in a shuffled order, checking after every removal.
            let mut order: Vec<usize> = (0..nodes.len()).collect();
            for i in (1..order.len()).rev() {
                let j = (rnd.next_u32() as usize) % (i + 1);
                order.swap(i, j);
            }
            for idx in order {
                let p: *mut Node = &mut nodes[idx];
                erase_augmented(&mut (*p).rb, root_ptr, &MaxAugment);
                let pos = keys.iter().position(|&k| k == (*p).key).unwrap();
                keys.remove(pos);
                check(root_ptr, &keys);
            }
        }
        assert!(keys.is_empty(), "round {round}");
    }
}

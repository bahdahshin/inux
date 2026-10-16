// SPDX-License-Identifier: GPL-2.0
//! Tests for the red-black tree, in the spirit of `lib/rbtree_test.c`.
//!
//! A `Node` embeds an `RbNode` and a key. The search for the insertion
//! slot is open-coded at the call site, as in C. After each mutation the
//! test checks the four red-black invariants and that an in-order walk is
//! sorted, which is what `check()` does in `rbtree_test.c`.

use rinux_lib::random32::RndState;
use rinux_lib::rbtree::*;

#[repr(C)]
struct Node {
    rb: RbNode,
    key: u32,
}

const OFFSET: usize = core::mem::offset_of!(Node, rb);

unsafe fn node_of(rb: *mut RbNode) -> *mut Node {
    // SAFETY: rb is the rb field of a live Node.
    unsafe { (rb as *mut u8).sub(OFFSET) as *mut Node }
}

/// Open-coded ordered insert (`insert()` in rbtree_test.c). Ties go right.
unsafe fn insert(node: *mut Node, root: *mut RbRoot) {
    // SAFETY: the tree and all its nodes are valid and pinned.
    unsafe {
        let mut new: *mut *mut RbNode = &mut (*root).rb_node;
        let mut parent: *mut RbNode = core::ptr::null_mut();
        let key = (*node).key;
        while !(*new).is_null() {
            parent = *new;
            let pkey = (*node_of(parent)).key;
            new = if key < pkey {
                &mut (*parent).rb_left
            } else {
                &mut (*parent).rb_right
            };
        }
        link_node(&mut (*node).rb, parent, new);
        insert_color(&mut (*node).rb, root);
    }
}

/// The RbNode is at a fixed offset, so its address reveals the colour bit
/// the test needs; expose a checker that walks the tree.
unsafe fn is_red(rb: *mut RbNode) -> bool {
    // The colour is private, so infer it the way rbtree_test.c's check()
    // does: a red node must have black children and a black parent. Here we
    // instead recompute black-heights and only need "not black"; expose it
    // by re-reading parent_color through a transmuted view.
    // SAFETY: RbNode's first field is the parent_color word.
    unsafe {
        let pc = *(rb as *const usize);
        pc & 1 == 0
    }
}

/// Checks the red-black invariants and returns the tree's size.
unsafe fn check(root: *mut RbRoot) -> usize {
    // SAFETY: valid tree.
    unsafe {
        // In-order walk must be sorted and visits every node once.
        let mut count = 0;
        let mut prev: Option<u32> = None;
        let mut n = first(root);
        while !n.is_null() {
            let key = (*node_of(n)).key;
            if let Some(p) = prev {
                assert!(p <= key, "in-order not sorted: {p} then {key}");
            }
            prev = Some(key);
            count += 1;

            // Property 4: a red node has no red child.
            if is_red(n) {
                let l = (*n).rb_left;
                let r = (*n).rb_right;
                assert!(l.is_null() || !is_red(l));
                assert!(r.is_null() || !is_red(r));
            }
            n = next(n);
        }

        // Property 5: every root-to-leaf path has the same number of black
        // nodes. Compute black-heights recursively.
        unsafe fn black_height(n: *mut RbNode) -> usize {
            // SAFETY: valid subtree.
            unsafe {
                if n.is_null() {
                    return 1;
                }
                let lh = black_height((*n).rb_left);
                let rh = black_height((*n).rb_right);
                assert_eq!(lh, rh, "black-height mismatch");
                lh + usize::from(!is_red(n))
            }
        }
        if !(*root).rb_node.is_null() {
            // Property 2: the root is black.
            assert!(!is_red((*root).rb_node));
            black_height((*root).rb_node);
        }
        count
    }
}

#[test]
fn insert_erase_random() {
    const N: usize = 500;
    let mut rng = RndState::new(1979);
    let mut nodes: Vec<Box<Node>> = (0..N)
        .map(|_| {
            Box::new(Node {
                rb: RbNode::new(),
                key: rng.next_u32() % 1000,
            })
        })
        .collect();
    let mut root = Box::new(RbRoot::new());
    let root_ptr: *mut RbRoot = &mut *root;

    // SAFETY: nodes outlive the tree and stay pinned in the Vec's boxes.
    unsafe {
        for (i, node) in nodes.iter_mut().enumerate() {
            insert(&mut **node, root_ptr);
            assert_eq!(check(root_ptr), i + 1);
        }

        // Erase in a shuffled order and keep checking.
        let mut order: Vec<usize> = (0..N).collect();
        for i in (1..N).rev() {
            let j = (rng.next_u32() as usize) % (i + 1);
            order.swap(i, j);
        }
        for (removed, &idx) in order.iter().enumerate() {
            erase(&mut nodes[idx].rb, root_ptr);
            assert_eq!(check(root_ptr), N - removed - 1);
        }
        assert!(root.is_empty());
    }
}

#[test]
fn ordered_iteration() {
    let keys = [50u32, 20, 70, 10, 30, 60, 80, 65, 5, 25];
    let mut nodes: Vec<Box<Node>> = keys
        .iter()
        .map(|&key| {
            Box::new(Node {
                rb: RbNode::new(),
                key,
            })
        })
        .collect();
    let mut root = Box::new(RbRoot::new());
    let root_ptr: *mut RbRoot = &mut *root;
    // SAFETY: nodes outlive the tree.
    unsafe {
        for node in nodes.iter_mut() {
            insert(&mut **node, root_ptr);
        }
        let mut sorted = keys;
        sorted.sort_unstable();

        let mut out = Vec::new();
        let mut n = first(root_ptr);
        while !n.is_null() {
            out.push((*node_of(n)).key);
            n = next(n);
        }
        assert_eq!(out, sorted);
    }
}

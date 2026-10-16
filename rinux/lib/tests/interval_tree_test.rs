// SPDX-License-Identifier: GPL-2.0-or-later
//! Host tests for the interval tree, checked against a brute-force overlap
//! scan over random intervals with random inserts, removals and queries.
//! Linux has no KUnit file for the generic interval tree code.

use rinux_lib::interval_tree::*;
use rinux_lib::random32::RndState;
use rinux_lib::rbtree::RbRoot;

/// Every node that overlaps `[start, last]`, found by walking the tree with
/// `iter_first` and `iter_next`.
unsafe fn query(root: *mut RbRoot, start: u64, last: u64) -> Vec<(u64, u64)> {
    let mut out = Vec::new();
    // SAFETY: the tree and the nodes are valid for the test.
    unsafe {
        let mut n = interval_tree_iter_first(root, start, last);
        while !n.is_null() {
            out.push(((*n).start, (*n).last));
            n = interval_tree_iter_next(n, start, last);
        }
    }
    out.sort_unstable();
    out
}

fn brute(live: &[(u64, u64)], start: u64, last: u64) -> Vec<(u64, u64)> {
    let mut out: Vec<(u64, u64)> = live
        .iter()
        .copied()
        .filter(|&(s, l)| s <= last && start <= l)
        .collect();
    out.sort_unstable();
    out
}

#[test]
fn random_inserts_removals_and_queries_match_brute_force() {
    let mut rnd = RndState::new(7);
    for _round in 0..10 {
        let mut nodes: Vec<IntervalNode> = Vec::new();
        for _ in 0..200 {
            let s = u64::from(rnd.next_u32() % 1000);
            let len = u64::from(rnd.next_u32() % 60);
            nodes.push(IntervalNode::new(s, s + len));
        }
        let mut root = RbRoot::new();
        let root_ptr: *mut RbRoot = &mut root;
        let mut live: Vec<(u64, u64)> = Vec::new();
        let mut present = vec![false; nodes.len()];
        unsafe {
            for i in 0..nodes.len() {
                let p: *mut IntervalNode = &mut nodes[i];
                interval_tree_insert(p, root_ptr);
                present[i] = true;
                live.push(((*p).start, (*p).last));
                // A few queries after each insert.
                for _ in 0..3 {
                    let a = u64::from(rnd.next_u32() % 1100);
                    let b = a + u64::from(rnd.next_u32() % 80);
                    assert_eq!(query(root_ptr, a, b), brute(&live, a, b), "insert {i}");
                }
            }
            // Remove half in a scattered order, querying as we go.
            for i in (0..nodes.len()).step_by(2) {
                let p: *mut IntervalNode = &mut nodes[i];
                interval_tree_remove(p, root_ptr);
                present[i] = false;
                let key = ((*p).start, (*p).last);
                let pos = live.iter().position(|&v| v == key).unwrap();
                live.remove(pos);
                let a = u64::from(rnd.next_u32() % 1100);
                let b = a + u64::from(rnd.next_u32() % 80);
                assert_eq!(query(root_ptr, a, b), brute(&live, a, b), "remove {i}");
            }
            // The whole range returns everything still present.
            assert_eq!(query(root_ptr, 0, u64::MAX), brute(&live, 0, u64::MAX));
        }
        assert!(present.iter().enumerate().all(|(i, &p)| p == (i % 2 == 1)));
    }
}

#[test]
fn an_empty_tree_and_a_miss_return_nothing() {
    let mut root = RbRoot::new();
    let root_ptr: *mut RbRoot = &mut root;
    unsafe {
        assert!(interval_tree_iter_first(root_ptr, 0, 10).is_null());
        let mut n = IntervalNode::new(10, 20);
        interval_tree_insert(&mut n, root_ptr);
        assert!(interval_tree_iter_first(root_ptr, 0, 9).is_null());
        assert!(interval_tree_iter_first(root_ptr, 21, 30).is_null());
        assert_eq!(query(root_ptr, 20, 20), vec![(10, 20)]);
        interval_tree_remove(&mut n, root_ptr);
    }
}

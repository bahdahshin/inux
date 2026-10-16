// SPDX-License-Identifier: GPL-2.0-only
//! Host tests for `lib/klist.c` behaviour: ordering, deletion under iteration
//! and deferred unlinking while a reference is held.

use rinux_lib::klist::{Klist, KlistIter, KlistNode};

fn order(k: &mut Klist, nodes: &mut [KlistNode]) -> Vec<usize> {
    let mut it = KlistIter::new();
    let mut out = Vec::new();
    while let Some(n) = it.next(k, nodes) {
        out.push(n);
    }
    out
}

#[test]
fn add_head_and_tail_order() {
    let mut nodes = [KlistNode::new(); 4];
    let mut k = Klist::new();
    k.add_tail(&mut nodes, 0);
    k.add_tail(&mut nodes, 1);
    k.add_head(&mut nodes, 2);
    assert_eq!(order(&mut k, &mut nodes), vec![2, 0, 1]);
}

#[test]
fn add_behind_and_before() {
    let mut nodes = [KlistNode::new(); 5];
    let mut k = Klist::new();
    k.add_tail(&mut nodes, 0);
    k.add_tail(&mut nodes, 1);
    k.add_behind(&mut nodes, 2, 0); // 0 2 1
    k.add_before(&mut nodes, 3, 0); // 3 0 2 1
    k.add_behind(&mut nodes, 4, 1); // 3 0 2 1 4
    assert_eq!(order(&mut k, &mut nodes), vec![3, 0, 2, 1, 4]);
}

#[test]
fn del_unlinks_when_unreferenced() {
    let mut nodes = [KlistNode::new(); 3];
    let mut k = Klist::new();
    for n in 0..3 {
        k.add_tail(&mut nodes, n);
    }
    k.del(&mut nodes, 1);
    assert!(!Klist::node_attached(&nodes, 1));
    assert_eq!(order(&mut k, &mut nodes), vec![0, 2]);
}

#[test]
fn held_node_stays_linked_until_released() {
    let mut nodes = [KlistNode::new(); 3];
    let mut k = Klist::new();
    for n in 0..3 {
        k.add_tail(&mut nodes, n);
    }
    let mut it = KlistIter::new();
    assert_eq!(it.next(&mut k, &mut nodes), Some(0));
    assert_eq!(it.next(&mut k, &mut nodes), Some(1));
    // Delete the node the iterator is on: still linked, but dead.
    k.del(&mut nodes, 1);
    assert!(Klist::node_attached(&nodes, 1));
    assert!(nodes[1].is_dead());
    // Advancing skips the dead node and releases it.
    assert_eq!(it.next(&mut k, &mut nodes), Some(2));
    assert!(!Klist::node_attached(&nodes, 1));
    assert_eq!(it.next(&mut k, &mut nodes), None);
}

#[test]
fn iterator_skips_dead_nodes() {
    let mut nodes = [KlistNode::new(); 3];
    let mut k = Klist::new();
    for n in 0..3 {
        k.add_tail(&mut nodes, n);
    }
    k.del(&mut nodes, 1);
    assert_eq!(order(&mut k, &mut nodes), vec![0, 2]);
}

#[test]
fn exit_releases_current_node() {
    let mut nodes = [KlistNode::new(); 2];
    let mut k = Klist::new();
    k.add_tail(&mut nodes, 0);
    k.add_tail(&mut nodes, 1);
    k.del(&mut nodes, 0);
    let mut it = KlistIter::new();
    assert_eq!(it.next(&mut k, &mut nodes), Some(1));
    assert_eq!(nodes[1].refs(), 2);
    it.exit(&mut k, &mut nodes);
    assert_eq!(nodes[1].refs(), 1);
    assert_eq!(nodes[0].refs(), 0);
    assert!(!Klist::node_attached(&nodes, 0));
    assert!(Klist::node_attached(&nodes, 1));
}

// SPDX-License-Identifier: GPL-2.0
//! Tests for the ported lock-less list, in the spirit of
//! `lib/tests/llist_kunit.c` (single-threaded: the lock-free paths are
//! exercised for correctness, not contention).

use rinux_lib::llist::*;

#[repr(C)]
struct Entry {
    node: LlistNode,
    value: i32,
}

const OFFSET: usize = core::mem::offset_of!(Entry, node);

unsafe fn value_of(n: *mut LlistNode) -> i32 {
    unsafe { (*(n.byte_sub(OFFSET) as *mut Entry)).value }
}

/// Collects the values along a detached chain.
unsafe fn values(n: *mut LlistNode) -> Vec<i32> {
    unsafe { iter(n).map(|node| value_of(node)).collect() }
}

// The `Box` is deliberate: the lock-less list links raw pointers into these
// nodes, so each must keep a stable address independent of the `Vec`.
#[allow(clippy::vec_box)]
fn make(values: &[i32]) -> Vec<Box<Entry>> {
    values
        .iter()
        .map(|&value| {
            Box::new(Entry {
                node: LlistNode::new(),
                value,
            })
        })
        .collect()
}

#[test]
fn add_is_lifo_and_del_all_takes_everything() {
    let mut e = make(&[1, 2, 3]);
    let head = LlistHead::new();
    assert!(head.is_empty());
    for entry in e.iter_mut() {
        let n: *mut LlistNode = &mut entry.node;
        unsafe { head.add(n) };
    }
    assert!(!head.is_empty());
    // LIFO: 3 was added last, so it is at the front.
    let chain = head.del_all();
    assert_eq!(unsafe { values(chain) }, vec![3, 2, 1]);
    assert!(head.is_empty());
}

#[test]
fn add_returns_empty_flag() {
    let mut e = make(&[10, 20]);
    let head = LlistHead::new();
    let a: *mut LlistNode = &mut e[0].node;
    let b: *mut LlistNode = &mut e[1].node;
    // First add onto an empty list returns true.
    assert!(unsafe { head.add(a) });
    // Second add returns false (list was non-empty).
    assert!(!unsafe { head.add(b) });
}

#[test]
fn del_first_pops_lifo() {
    let mut e = make(&[1, 2, 3]);
    let head = LlistHead::new();
    for entry in e.iter_mut() {
        let n: *mut LlistNode = &mut entry.node;
        unsafe { head.add(n) };
    }
    assert_eq!(unsafe { value_of(head.del_first()) }, 3);
    assert_eq!(unsafe { value_of(head.del_first()) }, 2);
    assert_eq!(unsafe { value_of(head.del_first()) }, 1);
    assert!(unsafe { head.del_first() }.is_null());
}

#[test]
fn reverse_order_restores_insertion_order() {
    let mut e = make(&[1, 2, 3, 4]);
    let head = LlistHead::new();
    for entry in e.iter_mut() {
        let n: *mut LlistNode = &mut entry.node;
        unsafe { head.add(n) };
    }
    // del_all gives LIFO [4,3,2,1]; reversing gives insertion order.
    let chain = head.del_all();
    let rev = unsafe { llist_reverse_order(chain) };
    assert_eq!(unsafe { values(rev) }, vec![1, 2, 3, 4]);
}

#[test]
fn add_batch_pushes_a_chain() {
    // Build a 3-node chain by hand and push it as a batch.
    let mut e = make(&[7, 8, 9]);
    // Link 7 -> 8 -> 9.
    let n0: *mut LlistNode = &mut e[0].node;
    let n1: *mut LlistNode = &mut e[1].node;
    let n2: *mut LlistNode = &mut e[2].node;
    unsafe {
        (*n0).next = n1;
        (*n1).next = n2;
    }
    let head = LlistHead::new();
    assert!(unsafe { head.add_batch(n0, n2) });
    let chain = head.del_all();
    assert_eq!(unsafe { values(chain) }, vec![7, 8, 9]);
}

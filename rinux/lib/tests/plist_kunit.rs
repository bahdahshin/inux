// SPDX-License-Identifier: GPL-2.0
//! Tests for the ported plist, in the spirit of `lib/plist.c`'s self-test:
//! after a mix of priority-ordered inserts and deletes the node_list is
//! always sorted by priority and stable among equal priorities.

use rinux_lib::plist::*;

#[repr(C)]
struct Entry {
    node: PlistNode,
    seq: u32,
}

fn head() -> Box<PlistHead> {
    let mut h = Box::new(PlistHead {
        node_list: rinux_lib::list::ListHead::new(),
    });
    unsafe { h.init() };
    h
}

fn entry(prio: i32, seq: u32) -> Box<Entry> {
    let mut e = Box::new(Entry {
        node: PlistNode {
            prio: 0,
            prio_list: rinux_lib::list::ListHead::new(),
            node_list: rinux_lib::list::ListHead::new(),
        },
        seq,
    });
    unsafe { e.node.init(prio) };
    e
}

/// The (prio, seq) pairs along node_list, in order.
unsafe fn pairs(h: *mut PlistHead) -> Vec<(i32, u32)> {
    unsafe {
        iter(h)
            .map(|n| {
                let e = n.cast::<Entry>(); // node is the first field of Entry
                ((*e).node.prio, (*e).seq)
            })
            .collect()
    }
}

fn is_sorted_stable(p: &[(i32, u32)]) {
    for w in p.windows(2) {
        assert!(w[0].0 <= w[1].0, "not sorted: {:?} then {:?}", w[0], w[1]);
        if w[0].0 == w[1].0 {
            assert!(
                w[0].1 < w[1].1,
                "not stable at equal prio: {:?} {:?}",
                w[0],
                w[1]
            );
        }
    }
}

#[test]
fn inserts_keep_priority_order() {
    let mut h = head();
    let hp: *mut PlistHead = &mut *h;
    // Insert priorities in a jumbled order with duplicates.
    let order = [5, 1, 3, 1, 9, 3, 0, 7, 3, 1];
    let mut keep = Vec::new();
    for (seq, &prio) in order.iter().enumerate() {
        let mut e = entry(prio, seq as u32);
        unsafe { plist_add(&mut e.node, hp) };
        keep.push(e);
    }
    let p = unsafe { pairs(hp) };
    is_sorted_stable(&p);
    // First is the smallest priority (0).
    let first = unsafe { plist_first(hp) };
    assert_eq!(unsafe { (*first).prio }, 0);
    assert_eq!(
        p.iter().map(|x| x.0).collect::<Vec<_>>(),
        vec![0, 1, 1, 1, 3, 3, 3, 5, 7, 9]
    );
}

#[test]
fn delete_hands_off_prio_list() {
    let mut h = head();
    let hp: *mut PlistHead = &mut *h;
    // Three nodes at the same priority, plus neighbours.
    let mut e: Vec<Box<Entry>> = Vec::new();
    for (seq, prio) in [2, 5, 5, 5, 8].into_iter().enumerate() {
        let mut en = entry(prio, seq as u32);
        unsafe { plist_add(&mut en.node, hp) };
        e.push(en);
    }
    // Delete the first of the priority-5 run (index 1 in e): its prio_list
    // slot must pass to the next 5, keeping order intact.
    unsafe { plist_del(&mut e[1].node, hp) };
    let p = unsafe { pairs(hp) };
    is_sorted_stable(&p);
    assert_eq!(p.iter().map(|x| x.0).collect::<Vec<_>>(), vec![2, 5, 5, 8]);
    // Delete all the fives, then the ends.
    unsafe {
        plist_del(&mut e[2].node, hp);
        plist_del(&mut e[3].node, hp);
    }
    let p = unsafe { pairs(hp) };
    assert_eq!(p.iter().map(|x| x.0).collect::<Vec<_>>(), vec![2, 8]);
    is_sorted_stable(&p);
}

#[test]
fn randomized_stays_sorted() {
    let mut h = head();
    let hp: *mut PlistHead = &mut *h;
    let mut state = 0x9e3779b9u32;
    let mut rng = || {
        state = state.wrapping_mul(1664525).wrapping_add(1013904223);
        state
    };
    let mut keep: Vec<Box<Entry>> = Vec::new();
    let mut present: Vec<bool> = Vec::new();
    for seq in 0..300u32 {
        let prio = (rng() % 11) as i32;
        let mut e = entry(prio, seq);
        unsafe { plist_add(&mut e.node, hp) };
        keep.push(e);
        present.push(true);
        // Occasionally delete a still-present node (never twice).
        if seq % 3 == 0 {
            let live: Vec<usize> = present
                .iter()
                .enumerate()
                .filter(|&(_, &p)| p)
                .map(|(i, _)| i)
                .collect();
            if !live.is_empty() {
                let idx = live[(rng() as usize) % live.len()];
                unsafe { plist_del(&mut keep[idx].node, hp) };
                present[idx] = false;
            }
        }
        let p = unsafe { pairs(hp) };
        for w in p.windows(2) {
            assert!(w[0].0 <= w[1].0);
        }
    }
    // The number still on the list matches the number not deleted.
    let live_count = present.iter().filter(|&&p| p).count();
    assert_eq!(unsafe { pairs(hp) }.len(), live_count);
}

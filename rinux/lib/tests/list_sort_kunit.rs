// SPDX-License-Identifier: GPL-2.0
//! Tests for the ported `list_sort`, in the spirit of
//! `lib/tests/list_sort_kunit.c`: sort lists of boxed, pinned entries and
//! check the result is correctly ordered, a valid circular doubly-linked
//! list, and stable for equal keys.

use rinux_lib::list::*;

#[repr(C)]
struct Entry {
    link: ListHead,
    key: i32,
    /// A tie-breaker that is NOT part of the sort key, to check stability.
    seq: u32,
}

const OFFSET: usize = core::mem::offset_of!(Entry, link);

/// Builds a circular list from `keys` (boxed so the nodes stay pinned),
/// returning the head and the kept-alive entries.
// The `Box` is deliberate: the intrusive list stores raw pointers into these
// nodes, so each must keep a stable address independent of the `Vec`.
#[allow(clippy::vec_box)]
fn build(keys: &[i32]) -> (Box<ListHead>, Vec<Box<Entry>>) {
    let mut head = Box::new(ListHead::new());
    let head_ptr: *mut ListHead = &mut *head;
    unsafe { (*head_ptr).init() };

    let mut entries = Vec::new();
    for (i, &key) in keys.iter().enumerate() {
        let mut e = Box::new(Entry {
            link: ListHead::new(),
            key,
            seq: i as u32,
        });
        let link: *mut ListHead = &mut e.link;
        unsafe { list_add_tail(link, head_ptr) };
        entries.push(e);
    }
    (head, entries)
}

/// The (key, seq) pairs on the list, front to back.
unsafe fn pairs(head: *mut ListHead) -> Vec<(i32, u32)> {
    unsafe {
        iter(head)
            .map(|n| {
                let e = list_entry::<Entry>(n, OFFSET);
                ((*e).key, (*e).seq)
            })
            .collect()
    }
}

/// Checks that `head` is a consistent circular doubly-linked list and returns
/// its forward key sequence; also verifies the reverse (prev) walk matches.
unsafe fn verify_and_keys(head: *mut ListHead) -> Vec<i32> {
    unsafe {
        let fwd: Vec<(i32, u32)> = pairs(head);
        // Walk backwards via prev and confirm it is the reverse of forward.
        let mut rev = Vec::new();
        let mut p = (*head).prev;
        while p != head {
            let e = list_entry::<Entry>(p, OFFSET);
            rev.push(((*e).key, (*e).seq));
            p = (*p).prev;
        }
        rev.reverse();
        assert_eq!(fwd, rev, "prev links inconsistent with next links");
        fwd.into_iter().map(|(k, _)| k).collect()
    }
}

fn cmp(a: *mut ListHead, b: *mut ListHead) -> bool {
    // true if key(a) <= key(b)
    unsafe {
        let ka = (*list_entry::<Entry>(a, OFFSET)).key;
        let kb = (*list_entry::<Entry>(b, OFFSET)).key;
        ka <= kb
    }
}

#[test]
fn sorts_small() {
    let (mut head, _keep) = build(&[3, 1, 4, 1, 5, 9, 2, 6]);
    let head_ptr: *mut ListHead = &mut *head;
    unsafe { list_sort(head_ptr, cmp) };
    assert_eq!(
        unsafe { verify_and_keys(head_ptr) },
        vec![1, 1, 2, 3, 4, 5, 6, 9]
    );
}

#[test]
fn already_sorted_and_reversed() {
    for keys in [
        (0..50).collect::<Vec<i32>>(),
        (0..50).rev().collect::<Vec<i32>>(),
    ] {
        let (mut head, _keep) = build(&keys);
        let head_ptr: *mut ListHead = &mut *head;
        unsafe { list_sort(head_ptr, cmp) };
        let got = unsafe { verify_and_keys(head_ptr) };
        assert_eq!(got, (0..50).collect::<Vec<i32>>());
    }
}

#[test]
fn empty_and_single() {
    for keys in [vec![], vec![42]] {
        let (mut head, _keep) = build(&keys);
        let head_ptr: *mut ListHead = &mut *head;
        unsafe { list_sort(head_ptr, cmp) };
        assert_eq!(unsafe { verify_and_keys(head_ptr) }, keys);
    }
}

#[test]
fn stable_for_equal_keys() {
    // Keys with many duplicates; equal keys must keep their original order
    // (ascending seq), which is what stability guarantees.
    let keys: Vec<i32> = (0..200).map(|i| i % 5).collect();
    let (mut head, _keep) = build(&keys);
    let head_ptr: *mut ListHead = &mut *head;
    unsafe { list_sort(head_ptr, cmp) };
    let p = unsafe { pairs(head_ptr) };
    // Keys are non-decreasing...
    for w in p.windows(2) {
        assert!(w[0].0 <= w[1].0);
    }
    // ...and within each equal key, seq is strictly increasing.
    for w in p.windows(2) {
        if w[0].0 == w[1].0 {
            assert!(w[0].1 < w[1].1, "not stable: {:?} then {:?}", w[0], w[1]);
        }
    }
}

#[test]
fn larger_pseudo_random() {
    // A deterministic LCG so the test is reproducible.
    let mut state = 0x1234_5678u32;
    let keys: Vec<i32> = (0..1000)
        .map(|_| {
            state = state.wrapping_mul(1103515245).wrapping_add(12345);
            (state >> 16) as i32 % 100
        })
        .collect();
    let (mut head, _keep) = build(&keys);
    let head_ptr: *mut ListHead = &mut *head;
    unsafe { list_sort(head_ptr, cmp) };
    let got = unsafe { verify_and_keys(head_ptr) };
    let mut want = keys.clone();
    want.sort();
    assert_eq!(got, want);
}

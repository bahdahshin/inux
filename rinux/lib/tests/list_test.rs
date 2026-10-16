// SPDX-License-Identifier: GPL-2.0
//! Tests for the intrusive list, in the spirit of `lib/tests/list-test.c`.
//!
//! An `Entry` embeds a `ListHead`; the test boxes a vector of entries so
//! they stay pinned, then exercises the list through raw pointers.

use rinux_lib::list::*;

#[repr(C)]
struct Entry {
    link: ListHead,
    value: i32,
}

impl Entry {
    fn new(value: i32) -> Box<Entry> {
        Box::new(Entry {
            link: ListHead::new(),
            value,
        })
    }

    fn link(self: &mut Box<Entry>) -> *mut ListHead {
        &mut self.link
    }
}

const OFFSET: usize = core::mem::offset_of!(Entry, link);

/// Collects the values on the list from front to back.
unsafe fn values(head: *mut ListHead) -> Vec<i32> {
    // SAFETY: the test keeps every entry alive and pinned.
    unsafe {
        iter(head)
            .map(|n| (*list_entry::<Entry>(n, OFFSET)).value)
            .collect()
    }
}

#[test]
fn add_and_order() {
    let mut head = Box::new(ListHead::new());
    let head_ptr: *mut ListHead = &mut *head;
    // SAFETY: head is boxed and stays put.
    unsafe {
        head.init();
        assert!(head.is_empty());

        let mut a = Entry::new(1);
        let mut b = Entry::new(2);
        let mut c = Entry::new(3);

        // list_add_tail keeps insertion order.
        list_add_tail(a.link(), head_ptr);
        list_add_tail(b.link(), head_ptr);
        list_add_tail(c.link(), head_ptr);
        assert!(!head.is_empty());
        assert_eq!(values(head_ptr), [1, 2, 3]);

        // list_add puts a node at the front.
        let mut z = Entry::new(0);
        list_add(z.link(), head_ptr);
        assert_eq!(values(head_ptr), [0, 1, 2, 3]);

        // Remove the middle.
        list_del(b.link());
        assert_eq!(values(head_ptr), [0, 1, 3]);

        // list_del poisons the removed node's links with null.
        assert!(b.link.next.is_null());

        list_del(z.link());
        list_del(a.link());
        list_del(c.link());
        assert!(head.is_empty());
        assert_eq!(values(head_ptr), [] as [i32; 0]);
    }
}

#[test]
fn del_init_makes_empty_list() {
    let mut head = Box::new(ListHead::new());
    let head_ptr: *mut ListHead = &mut *head;
    // SAFETY: head and entry stay pinned.
    unsafe {
        head.init();
        let mut a = Entry::new(7);
        list_add_tail(a.link(), head_ptr);
        list_del_init(a.link());
        assert!(head.is_empty());
        // The removed node is now its own empty list.
        assert!(a.link.is_empty());
    }
}

#[test]
fn fifo_and_lifo() {
    let mut head = Box::new(ListHead::new());
    let head_ptr: *mut ListHead = &mut *head;
    let mut entries: Vec<Box<Entry>> = (0..32).map(Entry::new).collect();
    // SAFETY: entries outlive the list.
    unsafe {
        head.init();
        for e in entries.iter_mut() {
            list_add(e.link(), head_ptr); // push front
        }
        // Added at the front, so the list reads back in reverse.
        assert_eq!(values(head_ptr), (0..32).rev().collect::<Vec<_>>());
    }
}

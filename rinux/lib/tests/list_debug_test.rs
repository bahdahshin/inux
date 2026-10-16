// SPDX-License-Identifier: GPL-2.0
//! Tests for the list consistency checks, in the spirit of `lib/list_debug.c`.

use rinux_lib::list::ListHead;
use rinux_lib::list_debug::*;

/// A ring of three nodes `a -> b -> c -> a`, boxed so the addresses are stable.
struct Ring {
    a: Box<ListHead>,
    b: Box<ListHead>,
    c: Box<ListHead>,
}

fn ring() -> Ring {
    let mut r = Ring {
        a: Box::new(ListHead::new()),
        b: Box::new(ListHead::new()),
        c: Box::new(ListHead::new()),
    };
    let a: *mut ListHead = &mut *r.a;
    let b: *mut ListHead = &mut *r.b;
    let c: *mut ListHead = &mut *r.c;
    // SAFETY: a, b and c are distinct live boxes that outlive the ring.
    unsafe {
        (*a).next = b;
        (*a).prev = c;
        (*b).next = c;
        (*b).prev = a;
        (*c).next = a;
        (*c).prev = b;
    }
    r
}

fn p(n: &ListHead) -> *const ListHead {
    n as *const ListHead
}

#[test]
fn add_between_consistent_neighbours_passes() {
    let r = ring();
    let loose = Box::new(ListHead::new());
    // Insert `loose` between a and b: a's next is b and b's prev is a.
    let res = unsafe { list_add_valid_or_report(p(&loose), p(&r.a), p(&r.b)) };
    assert_eq!(res, Ok(()));
}

#[test]
fn add_checks_fire_in_c_order() {
    let r = ring();
    let loose = Box::new(ListHead::new());
    unsafe {
        assert_eq!(
            list_add_valid_or_report(p(&loose), std::ptr::null(), p(&r.b)),
            Err(ListCorruption::AddPrevNull)
        );
        assert_eq!(
            list_add_valid_or_report(p(&loose), p(&r.a), std::ptr::null()),
            Err(ListCorruption::AddNextNull)
        );
        // Adding a's neighbour pair with prev and next swapped breaks next->prev first.
        assert_eq!(
            list_add_valid_or_report(p(&loose), p(&r.b), p(&r.a)),
            Err(ListCorruption::AddNextPrevMismatch)
        );
        assert_eq!(
            list_add_valid_or_report(p(&r.a), p(&r.a), p(&r.b)),
            Err(ListCorruption::AddDoubleAdd)
        );
    }
}

#[test]
fn del_on_consistent_ring_passes() {
    let r = ring();
    assert_eq!(unsafe { list_del_entry_valid_or_report(p(&r.b)) }, Ok(()));
}

#[test]
fn del_detects_poison_and_broken_links() {
    let r = ring();
    let b: *mut ListHead = Box::into_raw(r.b);
    // SAFETY: b came from Box::into_raw and is reclaimed at the end of the test.
    unsafe {
        let saved_next = (*b).next;
        (*b).next = LIST_POISON1 as *mut ListHead;
        assert_eq!(
            list_del_entry_valid_or_report(b),
            Err(ListCorruption::DelNextPoison)
        );
        (*b).next = saved_next;

        let saved_prev = (*b).prev;
        (*b).prev = LIST_POISON2 as *mut ListHead;
        assert_eq!(
            list_del_entry_valid_or_report(b),
            Err(ListCorruption::DelPrevPoison)
        );
        (*b).prev = saved_prev;

        // Break a's next pointer so it no longer points back at b.
        let a = saved_prev;
        (*a).next = std::ptr::null_mut();
        assert_eq!(
            list_del_entry_valid_or_report(b),
            Err(ListCorruption::DelPrevNextMismatch)
        );

        drop(Box::from_raw(b));
    }
    // a, c still owned by r (only b was moved out).
    drop(r.a);
    drop(r.c);
}

#[test]
fn messages_name_the_check() {
    assert!(
        ListCorruption::DelNextPoison
            .message()
            .contains("LIST_POISON1")
    );
    assert!(
        ListCorruption::AddDoubleAdd
            .message()
            .contains("double add")
    );
}

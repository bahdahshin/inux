// SPDX-License-Identifier: GPL-2.0-or-later
//! Host tests for `min_heap`. Linux has no KUnit file for it; these check
//! that the heap keeps its order through every operation.

use rinux_lib::min_heap::MinHeap;

fn lt(a: &u32, b: &u32) -> bool {
    a < b
}

fn drain(heap: &mut MinHeap<'_, u32>) -> Vec<u32> {
    let mut out = Vec::new();
    while let Some(&top) = heap.peek() {
        out.push(top);
        assert!(heap.pop(&lt));
    }
    out
}

#[test]
fn pushes_come_out_sorted() {
    let mut storage = [0u32; 16];
    let mut heap = MinHeap::new(&mut storage);
    for v in [9, 3, 7, 1, 8, 2, 6, 4, 5, 0] {
        assert!(heap.push(v, &lt));
    }
    assert_eq!(drain(&mut heap), (0..10).collect::<Vec<_>>());
}

#[test]
fn heapify_keeps_a_valid_heap_valid() {
    let mut storage = [0u32; 8];
    let mut heap = MinHeap::new(&mut storage);
    for v in [5, 9, 1, 8, 3, 7, 2, 6] {
        heap.push(v, &lt);
    }
    heap.heapify_all(&lt);
    assert_eq!(drain(&mut heap), vec![1, 2, 3, 5, 6, 7, 8, 9]);
}

#[test]
fn pop_push_replaces_the_root() {
    let mut storage = [0u32; 8];
    let mut heap = MinHeap::new(&mut storage);
    for v in [4, 2, 6] {
        heap.push(v, &lt);
    }
    heap.pop_push(10, &lt);
    assert_eq!(drain(&mut heap), vec![4, 6, 10]);
}

#[test]
fn delete_removes_the_named_element() {
    let mut storage = [0u32; 8];
    let mut heap = MinHeap::new(&mut storage);
    for v in [1, 5, 2, 8, 4] {
        heap.push(v, &lt);
    }
    let idx = heap.as_slice().iter().position(|&v| v == 5).unwrap();
    assert!(heap.del(idx, &lt));
    assert_eq!(drain(&mut heap), vec![1, 2, 4, 8]);
}

#[test]
fn empty_pop_and_full_push_are_refused() {
    let mut storage = [0u32; 2];
    let mut heap = MinHeap::new(&mut storage);
    assert!(!heap.pop(&lt));
    assert!(heap.push(1, &lt));
    assert!(heap.push(2, &lt));
    assert!(heap.is_full());
    assert!(!heap.push(3, &lt));
}

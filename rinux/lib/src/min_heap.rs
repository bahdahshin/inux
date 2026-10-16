// SPDX-License-Identifier: GPL-2.0
//! A binary min-heap over a caller-owned slice.
//!
//! Ported from `lib/min_heap.c` and `include/linux/min_heap.h`. The C header
//! is generic over element size and reaches elements through byte offsets
//! and a `less()` callback. Here the heap is generic over `T`, and the
//! ordering is a `less(a, b)` closure passed to each operation, as in C.
//! The sift-down, sift-up, heapify, pop, pop-push, push and delete
//! algorithms follow the header step for step, so the layout matches C.
//!
//! The C `WARN_ONCE()` on an empty pop or a full push becomes a `false`
//! return. The heap lives in `data`, and `nr` counts the live elements.

/// `struct min_heap`: a heap of `T` stored in `data[..nr]`.
#[derive(Debug)]
pub struct MinHeap<'a, T> {
    data: &'a mut [T],
    nr: usize,
}

/// Parent index of `i` in index form, as `parent()` computes in C.
fn parent(i: usize) -> usize {
    (i - 1) / 2
}

impl<'a, T> MinHeap<'a, T> {
    /// `__min_heap_init()`: an empty heap over `data`.
    pub fn new(data: &'a mut [T]) -> Self {
        Self { data, nr: 0 }
    }

    /// Number of elements currently in the heap (`heap->nr`).
    pub fn len(&self) -> usize {
        self.nr
    }

    /// Whether the heap has no elements.
    pub fn is_empty(&self) -> bool {
        self.nr == 0
    }

    /// The live elements, in heap layout (not sorted).
    pub fn as_slice(&self) -> &[T] {
        &self.data[..self.nr]
    }

    /// `__min_heap_peek()`: the smallest element, if any.
    pub fn peek(&self) -> Option<&T> {
        if self.nr == 0 {
            None
        } else {
            Some(&self.data[0])
        }
    }

    /// `__min_heap_full()`: whether `nr` has reached the capacity (`size`).
    pub fn is_full(&self) -> bool {
        self.nr == self.data.len()
    }

    /// `__min_heap_sift_down()`: move the element at `pos` down to where it
    /// belongs, following the smaller child at each level.
    pub fn sift_down<F: Fn(&T, &T) -> bool>(&mut self, pos: usize, less: &F) {
        let n = self.nr;
        let data = &mut *self.data;
        let a = pos;
        let mut b = a;
        // Find the sift-down path all the way to the leaves.
        let mut c = 2 * b + 1;
        let mut d = c + 1;
        while d < n {
            b = if less(&data[c], &data[d]) { c } else { d };
            c = 2 * b + 1;
            d = c + 1;
        }
        // Special case for the last leaf with no sibling.
        if d == n {
            b = c;
        }
        // Backtrack to the correct location.
        while b != a && less(&data[a], &data[b]) {
            b = parent(b);
        }
        // Shift the element into its correct place.
        let target = b;
        while b != a {
            b = parent(b);
            data.swap(b, target);
        }
    }

    /// `__min_heap_sift_up()`: move the element at `idx` up while it is
    /// smaller than its parent.
    pub fn sift_up<F: Fn(&T, &T) -> bool>(&mut self, idx: usize, less: &F) {
        let data = &mut *self.data;
        let mut a = idx;
        while a > 0 {
            let b = parent(a);
            if less(&data[b], &data[a]) {
                break;
            }
            data.swap(a, b);
            a = b;
        }
    }

    /// `__min_heapify_all()`: Floyd's O(n) heap construction over the
    /// current `nr` elements. Heaps built by `push` already satisfy the
    /// order, so this changes nothing for them.
    pub fn heapify_all<F: Fn(&T, &T) -> bool>(&mut self, less: &F) {
        for i in (0..self.nr / 2).rev() {
            self.sift_down(i, less);
        }
    }

    /// `__min_heap_pop()`: drop the smallest element. Returns `false` on an
    /// empty heap. The removed value is the one `peek()` showed.
    pub fn pop<F: Fn(&T, &T) -> bool>(&mut self, less: &F) -> bool {
        if self.nr == 0 {
            return false;
        }
        // Place the last element at the root and sift it down.
        self.nr -= 1;
        self.data.swap(0, self.nr);
        self.sift_down(0, less);
        true
    }

    /// `__min_heap_pop_push()`: replace the smallest element with `element`
    /// and sift it down: one sift instead of a pop and a push.
    pub fn pop_push<F: Fn(&T, &T) -> bool>(&mut self, element: T, less: &F) {
        self.data[0] = element;
        self.sift_down(0, less);
    }

    /// `__min_heap_push()`: add `element`. Returns `false` when the heap is
    /// full.
    pub fn push<F: Fn(&T, &T) -> bool>(&mut self, element: T, less: &F) -> bool {
        if self.is_full() {
            return false;
        }
        let pos = self.nr;
        self.data[pos] = element;
        self.nr += 1;
        self.sift_up(pos, less);
        true
    }

    /// `__min_heap_del()`: remove the element at `idx`, keeping the heap
    /// ordered. Returns `false` on an empty heap.
    pub fn del<F: Fn(&T, &T) -> bool>(&mut self, idx: usize, less: &F) -> bool {
        if self.nr == 0 {
            return false;
        }
        // Place the last element where `idx` was, then fix it up or down.
        self.nr -= 1;
        if idx == self.nr {
            return true;
        }
        self.data.swap(idx, self.nr);
        self.sift_up(idx, less);
        self.sift_down(idx, less);
        true
    }
}

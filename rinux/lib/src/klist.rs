// SPDX-License-Identifier: GPL-2.0-only
//! Reference-counted lists that can be modified during iteration.
//!
//! A port of `lib/klist.c` and `include/linux/klist.h`. The C version links
//! `struct klist_node` objects with `struct list_head` and counts users with
//! a `kref`. Here the nodes live in a caller-owned slice and the links are
//! indices into it, as the crate's conventions say; [`NIL`] ends a link.
//!
//! A node is removed with [`Klist::del`], which marks it dead and drops the
//! list's reference. The node is unlinked only when its count reaches zero,
//! so an iterator that holds it keeps it in place. [`KlistIter::next`] skips
//! dead nodes and takes a reference on the one it returns.
//!
//! The blocking `klist_remove()` (which waits for the last user) and the
//! `get`/`put` callbacks of `klist_init()` are not ported: the callers here
//! do their own counting, and nothing in this crate sleeps.

/// The "no node" link.
pub const NIL: usize = usize::MAX;

/// `struct klist_node`: one entry, owned by the caller's slice.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KlistNode {
    prev: usize,
    next: usize,
    /// `n_ref`: the list's own reference plus one per iterator.
    refs: u32,
    /// The `KNODE_DEAD` bit: deleted, waiting for the last reference.
    dead: bool,
    /// Whether the node is linked into a list.
    attached: bool,
}

impl KlistNode {
    /// An unattached node, ready for one of the `Klist::add_*` calls.
    pub const fn new() -> Self {
        Self {
            prev: NIL,
            next: NIL,
            refs: 0,
            dead: false,
            attached: false,
        }
    }

    /// `knode_dead()`.
    pub const fn is_dead(&self) -> bool {
        self.dead
    }

    /// The reference count, for tests and debugging.
    pub const fn refs(&self) -> u32 {
        self.refs
    }
}

impl Default for KlistNode {
    fn default() -> Self {
        Self::new()
    }
}

/// `struct klist`: the head of a list of [`KlistNode`]s.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Klist {
    first: usize,
    last: usize,
}

impl Default for Klist {
    fn default() -> Self {
        Self::new()
    }
}

impl Klist {
    /// `klist_init()`.
    pub const fn new() -> Self {
        Self {
            first: NIL,
            last: NIL,
        }
    }

    /// Whether the list holds no nodes.
    pub const fn is_empty(&self) -> bool {
        self.first == NIL
    }

    /// `klist_add_head()`: start a fresh node at the front.
    pub fn add_head(&mut self, nodes: &mut [KlistNode], n: usize) {
        self.init_node(nodes, n);
        nodes[n].next = self.first;
        if self.first != NIL {
            nodes[self.first].prev = n;
        } else {
            self.last = n;
        }
        self.first = n;
    }

    /// `klist_add_tail()`: start a fresh node at the back.
    pub fn add_tail(&mut self, nodes: &mut [KlistNode], n: usize) {
        self.init_node(nodes, n);
        nodes[n].prev = self.last;
        if self.last != NIL {
            nodes[self.last].next = n;
        } else {
            self.first = n;
        }
        self.last = n;
    }

    /// `klist_add_behind()`: start a fresh node after `pos`, which must be
    /// attached to this list.
    pub fn add_behind(&mut self, nodes: &mut [KlistNode], n: usize, pos: usize) {
        debug_assert!(nodes[pos].attached);
        if nodes[pos].next == NIL {
            self.add_tail(nodes, n);
            return;
        }
        self.init_node(nodes, n);
        let after = nodes[pos].next;
        nodes[n].prev = pos;
        nodes[n].next = after;
        nodes[after].prev = n;
        nodes[pos].next = n;
    }

    /// `klist_add_before()`: start a fresh node before `pos`, which must be
    /// attached to this list.
    pub fn add_before(&mut self, nodes: &mut [KlistNode], n: usize, pos: usize) {
        debug_assert!(nodes[pos].attached);
        if nodes[pos].prev == NIL {
            self.add_head(nodes, n);
            return;
        }
        self.init_node(nodes, n);
        let before = nodes[pos].prev;
        nodes[n].next = pos;
        nodes[n].prev = before;
        nodes[before].next = n;
        nodes[pos].prev = n;
    }

    /// `klist_del()`: mark the node dead and drop the list's reference.
    pub fn del(&mut self, nodes: &mut [KlistNode], n: usize) {
        debug_assert!(!nodes[n].dead, "klist node deleted twice");
        nodes[n].dead = true;
        self.put(nodes, n);
    }

    /// `klist_node_attached()`.
    pub const fn node_attached(nodes: &[KlistNode], n: usize) -> bool {
        nodes[n].attached
    }

    /// Give a node its first reference and link it at the back. The public
    /// add functions link it themselves; this covers `klist_node_init()`.
    fn init_node(&mut self, nodes: &mut [KlistNode], n: usize) {
        nodes[n] = KlistNode {
            prev: NIL,
            next: NIL,
            refs: 1,
            dead: false,
            attached: true,
        };
    }

    /// `klist_put()`: drop one reference, and unlink the node when none
    /// remain.
    fn put(&mut self, nodes: &mut [KlistNode], n: usize) {
        nodes[n].refs -= 1;
        if nodes[n].refs == 0 {
            self.unlink(nodes, n);
        }
    }

    /// `klist_release()`: take the node out of the list.
    fn unlink(&mut self, nodes: &mut [KlistNode], n: usize) {
        let (prev, next) = (nodes[n].prev, nodes[n].next);
        if prev != NIL {
            nodes[prev].next = next;
        } else {
            self.first = next;
        }
        if next != NIL {
            nodes[next].prev = prev;
        } else {
            self.last = prev;
        }
        nodes[n].prev = NIL;
        nodes[n].next = NIL;
        nodes[n].attached = false;
    }
}

/// `struct klist_iter`: a cursor that holds a reference on its current node.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KlistIter {
    cur: usize,
}

impl Default for KlistIter {
    fn default() -> Self {
        Self::new()
    }
}

impl KlistIter {
    /// `klist_iter_init()`: a cursor before the first node.
    pub const fn new() -> Self {
        Self { cur: NIL }
    }

    /// `klist_next()`: the next live node after the current one, or `None`
    /// at the end. The returned node gains a reference; the previous one
    /// loses its own.
    pub fn next(&mut self, k: &mut Klist, nodes: &mut [KlistNode]) -> Option<usize> {
        // Find the candidate before dropping the old reference, so the old
        // node can be unlinked without losing its successor.
        let mut cand = if self.cur == NIL {
            k.first
        } else {
            nodes[self.cur].next
        };
        while cand != NIL && nodes[cand].dead {
            cand = nodes[cand].next;
        }
        if self.cur != NIL {
            k.put(nodes, self.cur);
            self.cur = NIL;
        }
        if cand == NIL {
            return None;
        }
        nodes[cand].refs += 1;
        self.cur = cand;
        Some(cand)
    }

    /// `klist_iter_exit()`: drop the reference the cursor holds.
    pub fn exit(&mut self, k: &mut Klist, nodes: &mut [KlistNode]) {
        if self.cur != NIL {
            k.put(nodes, self.cur);
            self.cur = NIL;
        }
    }
}

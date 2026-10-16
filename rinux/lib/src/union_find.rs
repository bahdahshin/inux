// SPDX-License-Identifier: GPL-2.0
//! Union-find (disjoint sets) with path compression and union by rank.
//!
//! Ported from `lib/union_find.c` and `include/linux/union_find.h`. C nodes
//! are linked by pointer; here the nodes live in a caller-owned slice and
//! the parent links are indices into it, as the crate's conventions say.
//! Node `i` starts as a singleton set: call [`UfNode::new`] for each slot.

/// One element of a union-find forest: `struct uf_node`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UfNode {
    /// Index of the parent node; a root points at itself.
    pub parent: usize,
    /// Upper bound on the height of the tree rooted here.
    pub rank: u32,
}

impl UfNode {
    /// `uf_node_init()` for the node at index `index`.
    pub const fn new(index: usize) -> Self {
        Self {
            parent: index,
            rank: 0,
        }
    }
}

/// `uf_find()`: the root of `node`'s set, halving the path as it goes.
///
/// Panics if `node` or a parent link is out of bounds, as the C code would
/// fault on a bad pointer.
pub fn uf_find(nodes: &mut [UfNode], mut node: usize) -> usize {
    while nodes[node].parent != node {
        let parent = nodes[node].parent;
        nodes[node].parent = nodes[parent].parent;
        node = parent;
    }
    node
}

/// `uf_union()`: merge the sets containing `node1` and `node2`.
pub fn uf_union(nodes: &mut [UfNode], node1: usize, node2: usize) {
    let root1 = uf_find(nodes, node1);
    let root2 = uf_find(nodes, node2);
    if root1 == root2 {
        return;
    }
    if nodes[root1].rank < nodes[root2].rank {
        nodes[root1].parent = root2;
    } else if nodes[root1].rank > nodes[root2].rank {
        nodes[root2].parent = root1;
    } else {
        nodes[root2].parent = root1;
        nodes[root1].rank += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn forest<const N: usize>() -> [UfNode; N] {
        core::array::from_fn(UfNode::new)
    }

    #[test]
    fn singletons_are_their_own_root() {
        let mut nodes = forest::<4>();
        for i in 0..4 {
            assert_eq!(uf_find(&mut nodes, i), i);
        }
    }

    #[test]
    fn union_joins_sets_and_is_idempotent() {
        let mut nodes = forest::<4>();
        uf_union(&mut nodes, 0, 1);
        assert_eq!(uf_find(&mut nodes, 0), uf_find(&mut nodes, 1));
        assert_ne!(uf_find(&mut nodes, 0), uf_find(&mut nodes, 2));
        let root = uf_find(&mut nodes, 0);
        let rank = nodes[root].rank;
        uf_union(&mut nodes, 1, 0);
        assert_eq!(uf_find(&mut nodes, 0), root);
        assert_eq!(nodes[root].rank, rank);
    }

    #[test]
    fn union_by_rank_keeps_trees_shallow() {
        // Chain 0-1, 2-3, then join the two pairs: ranks tie, so the root of
        // the second pair goes under the first and the rank grows once.
        let mut nodes = forest::<4>();
        uf_union(&mut nodes, 0, 1);
        uf_union(&mut nodes, 2, 3);
        uf_union(&mut nodes, 0, 2);
        let root = uf_find(&mut nodes, 0);
        assert_eq!(nodes[root].rank, 2);
        for i in 0..4 {
            assert_eq!(uf_find(&mut nodes, i), root);
        }
    }

    #[test]
    fn find_compresses_paths() {
        // Build a chain 3 -> 2 -> 1 -> 0 by hand, then find on the bottom.
        let mut nodes = forest::<4>();
        nodes[3].parent = 2;
        nodes[2].parent = 1;
        nodes[1].parent = 0;
        assert_eq!(uf_find(&mut nodes, 3), 0);
        // Path halving: 3 now skips past 2, and the walk is shorter next time.
        assert_eq!(nodes[3].parent, 1);
        assert_eq!(uf_find(&mut nodes, 3), 0);
        assert_eq!(nodes[3].parent, 0);
    }

    #[test]
    fn random_unions_match_a_naive_partition() {
        const N: usize = 64;
        let mut nodes = forest::<N>();
        let mut label: [usize; N] = core::array::from_fn(|i| i);
        let mut seed: u64 = 0x9e37_79b9_7f4a_7c15;
        let mut next = || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed
        };
        for _ in 0..200 {
            let a = (next() % N as u64) as usize;
            let b = (next() % N as u64) as usize;
            uf_union(&mut nodes, a, b);
            let (la, lb) = (label[a], label[b]);
            for l in label.iter_mut() {
                if *l == lb {
                    *l = la;
                }
            }
        }
        for a in 0..N {
            for b in 0..N {
                let same_uf = uf_find(&mut nodes, a) == uf_find(&mut nodes, b);
                assert_eq!(same_uf, label[a] == label[b], "pair {a}, {b}");
            }
        }
    }
}

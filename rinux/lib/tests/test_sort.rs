// SPDX-License-Identifier: GPL-2.0-only
//! Port of `lib/tests/test_sort.c`, plus tests for `sort()` with a custom
//! swap and for `bsearch()`, which have no C tests.

use core::cmp::Ordering;

use rinux_lib::bsearch::bsearch;
use rinux_lib::sort::{sort, sort_with_swap};

const TEST_LEN: usize = 1000;

/// `cmpint()`: the C comparator returns `*a - *b`.
fn cmpint(a: &i32, b: &i32) -> Ordering {
    (a - b).cmp(&0)
}

/// The C test's generator. `r * 725861` overflows `int`; the kernel builds
/// with -fwrapv, so it wraps.
fn next(r: i32) -> i32 {
    r.wrapping_mul(725_861) % 6599
}

#[test]
fn test_sort() {
    let mut a = vec![0i32; TEST_LEN];
    let mut r = 1;
    for x in a.iter_mut() {
        r = next(r);
        *x = r;
    }
    sort(&mut a, cmpint);
    for i in 0..TEST_LEN - 1 {
        assert!(
            a[i] <= a[i + 1],
            "a[{i}] = {} > a[{}] = {}",
            a[i],
            i + 1,
            a[i + 1]
        );
    }

    r = 48;
    for x in a[..TEST_LEN - 1].iter_mut() {
        r = next(r);
        *x = r;
    }
    sort(&mut a[..TEST_LEN - 1], cmpint);
    for i in 0..TEST_LEN - 2 {
        assert!(a[i] <= a[i + 1]);
    }
}

#[test]
fn sort_matches_std_for_all_small_sizes() {
    let mut seed: u32 = 12345;
    for len in 0..200 {
        for _ in 0..5 {
            let v: Vec<u32> = (0..len)
                .map(|_| {
                    seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12345);
                    seed >> 20
                })
                .collect();
            let mut ours = v.clone();
            let mut want = v;
            sort(&mut ours, |a, b| a.cmp(b));
            want.sort_unstable();
            assert_eq!(ours, want, "len {len}");
        }
    }
}

#[test]
fn sort_custom_swap_keeps_index_in_step() {
    // A swap function that also maintains an inverse index, the kind of
    // auxiliary fix-up that custom swap functions exist for.
    let keys: Vec<u32> = (0..300).map(|i| (i * 7919) % 300).collect();
    let mut data: Vec<(u32, usize)> = keys.iter().enumerate().map(|(i, &k)| (k, i)).collect();
    let mut where_is: Vec<usize> = (0..data.len()).collect();
    let mut swaps = 0;

    sort_with_swap(
        &mut data,
        |a, b| a.0.cmp(&b.0),
        |s, i, j| {
            s.swap(i, j);
            where_is[s[i].1] = i;
            where_is[s[j].1] = j;
            swaps += 1;
        },
    );

    assert!(swaps > 0);
    assert!(data.windows(2).all(|w| w[0].0 <= w[1].0));
    for (pos, &(_, orig)) in data.iter().enumerate() {
        assert_eq!(where_is[orig], pos);
    }
}

#[test]
fn bsearch_finds_every_element() {
    let v: Vec<u32> = (0..1000).map(|i| i * 3).collect();
    for (i, x) in v.iter().enumerate() {
        assert_eq!(bsearch(x, &v, |k, e| k.cmp(e)), Some(i));
        assert_eq!(bsearch(&(x + 1), &v, |k, e| k.cmp(e)), None);
    }
    assert_eq!(bsearch(&5u32, &[] as &[u32], |k, e| k.cmp(e)), None);
}

#[test]
fn bsearch_key_of_another_type() {
    struct Sym {
        name: &'static str,
        addr: u64,
    }
    let table = [
        Sym {
            name: "alpha",
            addr: 1,
        },
        Sym {
            name: "beta",
            addr: 2,
        },
        Sym {
            name: "gamma",
            addr: 3,
        },
        Sym {
            name: "delta",
            addr: 4,
        },
    ];
    let mut sorted: Vec<&Sym> = table.iter().collect();
    sort(&mut sorted, |a, b| a.name.cmp(b.name));
    let i = bsearch("gamma", &sorted, |k: &str, e| k.cmp(e.name)).unwrap();
    assert_eq!(sorted[i].addr, 3);
    assert_eq!(bsearch("omega", &sorted, |k: &str, e| k.cmp(e.name)), None);
}

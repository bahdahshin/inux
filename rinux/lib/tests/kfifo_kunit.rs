// SPDX-License-Identifier: GPL-2.0
//! Tests for the ported byte kfifo.
//!
//! Exercises enqueue/dequeue, the fill-then-drain path, wraparound across the
//! buffer end (the two-copy split), peek without consuming, and the bounded
//! behaviour when the FIFO fills or empties.

use rinux_lib::kfifo::Kfifo;

#[test]
fn basic_put_get() {
    let mut f = Kfifo::<8>::new();
    assert!(f.is_empty());
    assert_eq!(f.unused(), 8);
    assert_eq!(f.put(b"abcd"), 4);
    assert_eq!(f.len(), 4);
    let mut out = [0u8; 4];
    assert_eq!(f.get(&mut out), 4);
    assert_eq!(&out, b"abcd");
    assert!(f.is_empty());
}

#[test]
fn fills_up_and_rejects_excess() {
    let mut f = Kfifo::<4>::new();
    // Only 4 bytes fit.
    assert_eq!(f.put(b"abcdef"), 4);
    assert!(f.is_full());
    assert_eq!(f.put(b"x"), 0);
    let mut out = [0u8; 8];
    assert_eq!(f.get(&mut out), 4);
    assert_eq!(&out[..4], b"abcd");
}

#[test]
fn wraps_around_the_buffer_end() {
    let mut f = Kfifo::<8>::new();
    // Advance in/out so the next write straddles the end of the buffer.
    f.put(b"123456");
    let mut tmp = [0u8; 6];
    assert_eq!(f.get(&mut tmp), 6); // out is now at 6, in at 6
    // This 5-byte write wraps: 2 bytes at [6,7], 3 bytes at [0,1,2].
    assert_eq!(f.put(b"ABCDE"), 5);
    assert_eq!(f.len(), 5);
    let mut out = [0u8; 5];
    assert_eq!(f.get(&mut out), 5);
    assert_eq!(&out, b"ABCDE");
}

#[test]
fn peek_does_not_consume() {
    let mut f = Kfifo::<16>::new();
    f.put(b"hello");
    let mut p = [0u8; 5];
    assert_eq!(f.peek(&mut p), 5);
    assert_eq!(&p, b"hello");
    assert_eq!(f.len(), 5); // still there
    let mut g = [0u8; 5];
    assert_eq!(f.get(&mut g), 5);
    assert_eq!(&g, b"hello");
}

#[test]
fn byte_helpers() {
    let mut f = Kfifo::<2>::new();
    assert!(f.put_byte(1));
    assert!(f.put_byte(2));
    assert!(!f.put_byte(3)); // full
    assert_eq!(f.get_byte(), Some(1));
    assert_eq!(f.get_byte(), Some(2));
    assert_eq!(f.get_byte(), None); // empty
}

#[test]
fn streaming_many_wraps() {
    // Push far more than the capacity through a small FIFO, one byte in and
    // one byte out, to exercise counter wraparound many times.
    let mut f = Kfifo::<4>::new();
    for i in 0..10_000u32 {
        assert!(f.put_byte(i as u8));
        assert_eq!(f.get_byte(), Some(i as u8));
    }
    assert!(f.is_empty());
}

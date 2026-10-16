// SPDX-License-Identifier: GPL-2.0
//! Host tests for the printk paths that the sink carries: the ratelimit
//! "callbacks suppressed" report (`lib/ratelimit.c`) and `seq_buf_do_printk()`
//! (`lib/seq_buf.c`).

use rinux_lib::printk::{LOGLEVEL_INFO, LOGLEVEL_WARNING};
use rinux_lib::ratelimit::RatelimitState;
use rinux_lib::seq_buf::SeqBuf;
use std::cell::RefCell;

#[test]
fn ratelimit_reports_the_suppressed_count_when_the_window_ends() {
    let mut rs = RatelimitState::new(100, 2);
    let lines = RefCell::new(Vec::<(i32, String)>::new());
    let mut sink = |lvl: i32, line: &[u8]| {
        lines
            .borrow_mut()
            .push((lvl, String::from_utf8(line.to_vec()).unwrap()))
    };

    assert!(rs.ratelimit_report(0, b"foo", &mut sink));
    assert!(rs.ratelimit_report(1, b"foo", &mut sink));
    assert!(!rs.ratelimit_report(2, b"foo", &mut sink));
    assert!(!rs.ratelimit_report(3, b"foo", &mut sink));
    assert!(
        lines.borrow().is_empty(),
        "nothing is reported inside the window"
    );

    // The window ends: the two suppressed events are reported, then this event
    // is allowed again.
    assert!(rs.ratelimit_report(200, b"foo", &mut sink));
    assert_eq!(
        *lines.borrow(),
        vec![(LOGLEVEL_WARNING, "foo: 2 callbacks suppressed".to_string())]
    );
    assert_eq!(rs.missed(), 0);
}

#[test]
fn ratelimit_reports_nothing_when_nothing_was_suppressed() {
    let mut rs = RatelimitState::new(100, 5);
    let mut lines = 0;
    let mut sink = |_: i32, _: &[u8]| lines += 1;
    assert!(rs.ratelimit_report(0, b"foo", &mut sink));
    assert!(rs.ratelimit_report(200, b"foo", &mut sink));
    assert_eq!(lines, 0);
}

#[test]
fn seq_buf_prints_each_line_and_the_unterminated_tail() {
    let mut mem = [0u8; 32];
    let mut s = SeqBuf::new(&mut mem);
    s.puts("first\nsecond\ntail").unwrap();
    let mut lines: Vec<(i32, String)> = Vec::new();
    s.do_printk(LOGLEVEL_INFO, &mut |lvl, line| {
        lines.push((lvl, String::from_utf8(line.to_vec()).unwrap()))
    });
    assert_eq!(
        lines,
        vec![
            (LOGLEVEL_INFO, "first".to_string()),
            (LOGLEVEL_INFO, "second".to_string()),
            (LOGLEVEL_INFO, "tail".to_string()),
        ]
    );
}

#[test]
fn seq_buf_empty_prints_nothing() {
    let mut mem = [0u8; 8];
    let s = SeqBuf::new(&mut mem);
    let mut count = 0;
    s.do_printk(LOGLEVEL_INFO, &mut |_, _| count += 1);
    assert_eq!(count, 0);
}

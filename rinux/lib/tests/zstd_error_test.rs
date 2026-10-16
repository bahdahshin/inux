// SPDX-License-Identifier: GPL-2.0+ OR BSD-3-Clause
//! Host tests for the zstd error list. Linux has no KUnit file for it; these
//! check every code's message, the unknown-code default, and the error range.

use rinux_lib::zstd_error::{MAX_CODE, code, err_get_error_string, get_error_code, is_error};

#[test]
fn every_named_code_has_its_message() {
    let table: &[(u32, &str)] = &[
        (code::NO_ERROR, "No error detected"),
        (code::GENERIC, "Error (generic)"),
        (code::PREFIX_UNKNOWN, "Unknown frame descriptor"),
        (code::CORRUPTION_DETECTED, "Data corruption detected"),
        (code::CHECKSUM_WRONG, "Restored data doesn't match checksum"),
        (
            code::MEMORY_ALLOCATION,
            "Allocation error : not enough memory",
        ),
        (code::DST_SIZE_TOO_SMALL, "Destination buffer is too small"),
        (code::SRC_SIZE_WRONG, "Src size is incorrect"),
        (
            code::NO_FORWARD_PROGRESS_DEST_FULL,
            "Operation made no progress over multiple calls, due to output buffer being full",
        ),
        (
            code::NO_FORWARD_PROGRESS_INPUT_EMPTY,
            "Operation made no progress over multiple calls, due to input being empty",
        ),
        (code::FRAME_INDEX_TOO_LARGE, "Frame index is too large"),
        (
            code::SEEKABLE_IO,
            "An I/O error occurred when reading/seeking",
        ),
        (
            code::EXTERNAL_SEQUENCES_INVALID,
            "External sequences are not valid",
        ),
    ];
    for &(c, msg) in table {
        assert_eq!(err_get_error_string(c), msg, "code {c}");
    }
}

#[test]
fn unnamed_codes_get_the_default_message() {
    // maxCode is never a real error, and neither is anything the list lacks.
    assert_eq!(
        err_get_error_string(MAX_CODE as u32),
        "Unspecified error code"
    );
    assert_eq!(err_get_error_string(2), "Unspecified error code");
    assert_eq!(err_get_error_string(u32::MAX), "Unspecified error code");
}

#[test]
fn messages_are_distinct() {
    let codes = [
        code::NO_ERROR,
        code::GENERIC,
        code::PREFIX_UNKNOWN,
        code::VERSION_UNSUPPORTED,
        code::CORRUPTION_DETECTED,
        code::CHECKSUM_WRONG,
        code::STAGE_WRONG,
        code::INIT_MISSING,
        code::DST_BUFFER_WRONG,
        code::SRC_BUFFER_WRONG,
    ];
    for (i, &a) in codes.iter().enumerate() {
        for &b in &codes[i + 1..] {
            assert_ne!(err_get_error_string(a), err_get_error_string(b));
        }
    }
}

#[test]
fn error_range_is_the_top_of_size_t() {
    // An error is stored as 0 - code, so every named code is in the error range
    // and a byte count below it is not.
    for c in [
        code::GENERIC,
        code::CORRUPTION_DETECTED,
        code::EXTERNAL_SEQUENCES_INVALID,
    ] {
        let stored = 0usize.wrapping_sub(c as usize);
        assert!(is_error(stored), "code {c}");
        assert_eq!(get_error_code(stored), c);
    }
    assert!(!is_error(0));
    assert!(!is_error(1 << 40));
    // The boundary is `0 - maxCode`, which is not an error (C's `>` test).
    assert!(!is_error(0usize.wrapping_sub(MAX_CODE)));
    assert!(is_error(0usize.wrapping_sub(MAX_CODE - 1)));
    // A byte count has no error code.
    assert_eq!(get_error_code(4096), 0);
}

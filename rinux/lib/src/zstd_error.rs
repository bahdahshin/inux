// SPDX-License-Identifier: GPL-2.0+ OR BSD-3-Clause
//! The zstd error codes and their one shared list of messages.
//!
//! Ported from `lib/zstd/common/error_private.c` (`ERR_getErrorString()`),
//! `lib/zstd/common/error_private.h` (`ERR_isError()`, `ERR_getErrorCode()`)
//! and the enum in `include/linux/zstd_errors.h`.
//!
//! zstd reports an error as a `size_t` counting down from `usize::MAX`: the
//! code `c` is stored as `0 - c`. A value above `ERROR(maxCode)` is an error;
//! everything else is a byte count. The codes are the C values, so the
//! messages match C's for every code the enum names.

/// `ZSTD_error_maxCode`: the code past the last one. C uses it to bound the
/// error range; it is never a real error.
pub const MAX_CODE: usize = 120;

/// `ZSTD_isError()`: whether a zstd return value is an error code.
pub const fn is_error(code: usize) -> bool {
    code > 0usize.wrapping_sub(MAX_CODE)
}

/// `ZSTD_getErrorCode()`: the error code of a return value, or 0 when the
/// value is a byte count.
pub const fn get_error_code(code: usize) -> u32 {
    if !is_error(code) {
        0
    } else {
        // Wrapping keeps the C `0 - code` for the in-range values.
        code.wrapping_neg() as u32
    }
}

/// `ZSTD_error_*` values from `include/linux/zstd_errors.h`, with the
/// unstable codes marked as in the header.
pub mod code {
    pub const NO_ERROR: u32 = 0;
    pub const GENERIC: u32 = 1;
    pub const PREFIX_UNKNOWN: u32 = 10;
    pub const VERSION_UNSUPPORTED: u32 = 12;
    pub const FRAME_PARAMETER_UNSUPPORTED: u32 = 14;
    pub const FRAME_PARAMETER_WINDOW_TOO_LARGE: u32 = 16;
    pub const CORRUPTION_DETECTED: u32 = 20;
    pub const CHECKSUM_WRONG: u32 = 22;
    pub const LITERALS_HEADER_WRONG: u32 = 24;
    pub const DICTIONARY_CORRUPTED: u32 = 30;
    pub const DICTIONARY_WRONG: u32 = 32;
    pub const DICTIONARY_CREATION_FAILED: u32 = 34;
    pub const PARAMETER_UNSUPPORTED: u32 = 40;
    pub const PARAMETER_COMBINATION_UNSUPPORTED: u32 = 41;
    pub const PARAMETER_OUT_OF_BOUND: u32 = 42;
    pub const TABLE_LOG_TOO_LARGE: u32 = 44;
    pub const MAX_SYMBOL_VALUE_TOO_LARGE: u32 = 46;
    pub const MAX_SYMBOL_VALUE_TOO_SMALL: u32 = 48;
    pub const CANNOT_PRODUCE_UNCOMPRESSED_BLOCK: u32 = 49;
    pub const STABILITY_CONDITION_NOT_RESPECTED: u32 = 50;
    pub const STAGE_WRONG: u32 = 60;
    pub const INIT_MISSING: u32 = 62;
    pub const MEMORY_ALLOCATION: u32 = 64;
    pub const WORKSPACE_TOO_SMALL: u32 = 66;
    pub const DST_SIZE_TOO_SMALL: u32 = 70;
    pub const SRC_SIZE_WRONG: u32 = 72;
    pub const DST_BUFFER_NULL: u32 = 74;
    pub const NO_FORWARD_PROGRESS_DEST_FULL: u32 = 80;
    pub const NO_FORWARD_PROGRESS_INPUT_EMPTY: u32 = 82;
    // Unstable: may be removed or changed in a future version.
    pub const FRAME_INDEX_TOO_LARGE: u32 = 100;
    pub const SEEKABLE_IO: u32 = 102;
    pub const DST_BUFFER_WRONG: u32 = 104;
    pub const SRC_BUFFER_WRONG: u32 = 105;
    pub const SEQUENCE_PRODUCER_FAILED: u32 = 106;
    pub const EXTERNAL_SEQUENCES_INVALID: u32 = 107;
}

/// `ERR_getErrorString()`: the message for an error code. Codes the list
/// does not name get "Unspecified error code", as the C `default` does.
/// (C's `ZSTD_STRIP_ERROR_STRINGS` build is not ported.)
pub const fn err_get_error_string(code: u32) -> &'static str {
    match code {
        code::NO_ERROR => "No error detected",
        code::GENERIC => "Error (generic)",
        code::PREFIX_UNKNOWN => "Unknown frame descriptor",
        code::VERSION_UNSUPPORTED => "Version not supported",
        code::FRAME_PARAMETER_UNSUPPORTED => "Unsupported frame parameter",
        code::FRAME_PARAMETER_WINDOW_TOO_LARGE => "Frame requires too much memory for decoding",
        code::CORRUPTION_DETECTED => "Data corruption detected",
        code::CHECKSUM_WRONG => "Restored data doesn't match checksum",
        code::LITERALS_HEADER_WRONG => {
            "Header of Literals' block doesn't respect format specification"
        }
        code::PARAMETER_UNSUPPORTED => "Unsupported parameter",
        code::PARAMETER_COMBINATION_UNSUPPORTED => "Unsupported combination of parameters",
        code::PARAMETER_OUT_OF_BOUND => "Parameter is out of bound",
        code::INIT_MISSING => "Context should be init first",
        code::MEMORY_ALLOCATION => "Allocation error : not enough memory",
        code::WORKSPACE_TOO_SMALL => "workSpace buffer is not large enough",
        code::STAGE_WRONG => "Operation not authorized at current processing stage",
        code::TABLE_LOG_TOO_LARGE => "tableLog requires too much memory : unsupported",
        code::MAX_SYMBOL_VALUE_TOO_LARGE => "Unsupported max Symbol Value : too large",
        code::MAX_SYMBOL_VALUE_TOO_SMALL => "Specified maxSymbolValue is too small",
        code::CANNOT_PRODUCE_UNCOMPRESSED_BLOCK => {
            "This mode cannot generate an uncompressed block"
        }
        code::STABILITY_CONDITION_NOT_RESPECTED => {
            "pledged buffer stability condition is not respected"
        }
        code::DICTIONARY_CORRUPTED => "Dictionary is corrupted",
        code::DICTIONARY_WRONG => "Dictionary mismatch",
        code::DICTIONARY_CREATION_FAILED => "Cannot create Dictionary from provided samples",
        code::DST_SIZE_TOO_SMALL => "Destination buffer is too small",
        code::SRC_SIZE_WRONG => "Src size is incorrect",
        code::DST_BUFFER_NULL => "Operation on NULL destination buffer",
        code::NO_FORWARD_PROGRESS_DEST_FULL => {
            "Operation made no progress over multiple calls, due to output buffer being full"
        }
        code::NO_FORWARD_PROGRESS_INPUT_EMPTY => {
            "Operation made no progress over multiple calls, due to input being empty"
        }
        code::FRAME_INDEX_TOO_LARGE => "Frame index is too large",
        code::SEEKABLE_IO => "An I/O error occurred when reading/seeking",
        code::DST_BUFFER_WRONG => "Destination buffer is wrong",
        code::SRC_BUFFER_WRONG => "Source buffer is wrong",
        code::SEQUENCE_PRODUCER_FAILED => {
            "Block-level external sequence producer returned an error code"
        }
        code::EXTERNAL_SEQUENCES_INVALID => "External sequences are not valid",
        _ => "Unspecified error code",
    }
}

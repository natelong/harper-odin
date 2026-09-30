#![allow(non_camel_case_types)]

use std::cell::RefCell;
use std::ffi::{c_char, CString};
use std::panic::AssertUnwindSafe;
use std::sync::Arc;

pub use harper_core;
use harper_core::linting::{LintGroup, LintKind, Linter, Suggestion};
use harper_core::parsers::PlainEnglish;
use harper_core::spell::FstDictionary;
use harper_core::{Dialect, Document, Span};

/// Reports the pinned harper-core version (e.g. "2.11.0").
pub const HARPER_CORE_VERSION: &str = "2.11.0";

/// Reports the C-ABI wrapper version (e.g. 1).
pub const HARPER_C_ABI_VERSION: u32 = 1;

// Dialects matching Harper_Dialect in harper_c.h
pub const HARPER_DIALECT_AMERICAN: u32 = 0;
pub const HARPER_DIALECT_AUSTRALIAN: u32 = 1;
pub const HARPER_DIALECT_BRITISH: u32 = 2;
pub const HARPER_DIALECT_CANADIAN: u32 = 3;

// Lint kinds matching Harper_Lint_Kind in harper_c.h
pub const HARPER_AGREEMENT: u8 = 0;
pub const HARPER_BOUNDARY_ERROR: u8 = 1;
pub const HARPER_CAPITALIZATION: u8 = 2;
pub const HARPER_EGGCORN: u8 = 3;
pub const HARPER_ENHANCEMENT: u8 = 4;
pub const HARPER_FORMATTING: u8 = 5;
pub const HARPER_GRAMMAR: u8 = 6;
pub const HARPER_MALAPROPISM: u8 = 7;
pub const HARPER_MISCELLANEOUS: u8 = 8;
pub const HARPER_NONSTANDARD: u8 = 9;
pub const HARPER_PUNCTUATION: u8 = 10;
pub const HARPER_READABILITY: u8 = 11;
pub const HARPER_REDUNDANCY: u8 = 12;
pub const HARPER_REGIONALISM: u8 = 13;
pub const HARPER_REPETITION: u8 = 14;
pub const HARPER_SPELLING: u8 = 15;
pub const HARPER_STYLE: u8 = 16;
pub const HARPER_TYPO: u8 = 17;
pub const HARPER_USAGE: u8 = 18;
pub const HARPER_WORD_CHOICE: u8 = 19;
pub const HARPER_WORD_ORDER: u8 = 20;

// Suggestion actions matching Harper_Suggestion_Kind in harper_c.h
pub const HARPER_SUGGESTION_REPLACE: u8 = 0;
pub const HARPER_SUGGESTION_INSERT_AFTER: u8 = 1;
pub const HARPER_SUGGESTION_REMOVE: u8 = 2;

/// Wire representation of an individual lint. Offsets are UTF-8 byte indices!
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Harper_C_Lint {
    pub start: u32,
    pub end: u32,
    pub kind: u8,
    pub priority: u8,
    pub suggestion_kind: u8,
    pub _pad: u8,
    pub message: *const c_char,
    pub suggestion: *const c_char,
}

/// Whole-result allocation block.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Harper_Lint_Result {
    pub lints: *mut Harper_C_Lint,
    pub count: usize,
}

/// Checker state wrapped behind an opaque handle.
pub struct Harper_State {
    pub lint_group: LintGroup,
    pub dictionary: Arc<FstDictionary>,
    pub dialect: Dialect,
}

// Ensure Harper_State implements Send so Harper_Handle can be used across background threads
unsafe impl Send for Harper_State {}

/// Opaque instance handle
pub type Harper_Handle = *mut Harper_State;

thread_local! {
    static LAST_ERROR: RefCell<Option<CString>> = const { RefCell::new(None) };
}

fn set_last_error(msg: impl Into<String>) {
    let s = msg.into().replace('\0', " ");
    let c = CString::new(s).unwrap_or_default();
    LAST_ERROR.with(|cell| {
        *cell.borrow_mut() = Some(c);
    });
}

fn clear_last_error() {
    LAST_ERROR.with(|cell| {
        *cell.borrow_mut() = None;
    });
}

/// Converts Harper's Unicode scalar value (character) spans into UTF-8 byte offsets.
pub fn char_span_to_byte_span(text: &str, span: Span<char>) -> Option<(usize, usize)> {
    let mut byte_start = None;
    let mut byte_end = None;
    let mut char_idx = 0;

    for (b_idx, _) in text.char_indices() {
        if char_idx == span.start {
            byte_start = Some(b_idx);
        }
        if char_idx == span.end {
            byte_end = Some(b_idx);
            break;
        }
        char_idx += 1;
    }
    if span.start == char_idx && byte_start.is_none() {
        byte_start = Some(text.len());
    }
    if span.end == char_idx && byte_end.is_none() {
        byte_end = Some(text.len());
    }

    match (byte_start, byte_end) {
        (Some(s), Some(e)) if s <= e && e <= text.len() => Some((s, e)),
        _ => None,
    }
}

pub fn map_dialect(dialect: u32) -> Result<Dialect, String> {
    match dialect {
        HARPER_DIALECT_AMERICAN => Ok(Dialect::American),
        HARPER_DIALECT_AUSTRALIAN => Ok(Dialect::Australian),
        HARPER_DIALECT_BRITISH => Ok(Dialect::British),
        HARPER_DIALECT_CANADIAN => Ok(Dialect::Canadian),
        other => Err(format!("Unsupported dialect ID: {other}")),
    }
}

pub fn map_lint_kind(kind: LintKind) -> u8 {
    match kind {
        LintKind::Agreement => HARPER_AGREEMENT,
        LintKind::BoundaryError => HARPER_BOUNDARY_ERROR,
        LintKind::Capitalization => HARPER_CAPITALIZATION,
        LintKind::Eggcorn => HARPER_EGGCORN,
        LintKind::Enhancement => HARPER_ENHANCEMENT,
        LintKind::Formatting => HARPER_FORMATTING,
        LintKind::Grammar => HARPER_GRAMMAR,
        LintKind::Malapropism => HARPER_MALAPROPISM,
        LintKind::Miscellaneous => HARPER_MISCELLANEOUS,
        LintKind::Nonstandard => HARPER_NONSTANDARD,
        LintKind::Punctuation => HARPER_PUNCTUATION,
        LintKind::Readability => HARPER_READABILITY,
        LintKind::Redundancy => HARPER_REDUNDANCY,
        LintKind::Regionalism => HARPER_REGIONALISM,
        LintKind::Repetition => HARPER_REPETITION,
        LintKind::Spelling => HARPER_SPELLING,
        LintKind::Style => HARPER_STYLE,
        LintKind::Typo => HARPER_TYPO,
        LintKind::Usage => HARPER_USAGE,
        LintKind::WordChoice => HARPER_WORD_CHOICE,
        LintKind::WordOrder => HARPER_WORD_ORDER,
    }
}

/// Reports the pinned harper-core version (e.g. "2.11.0").
#[no_mangle]
pub unsafe extern "C" fn harper_version() -> *const c_char {
    let result = std::panic::catch_unwind(|| c"2.11.0".as_ptr());
    match result {
        Ok(ptr) => ptr,
        Err(_) => {
            set_last_error("harper_version panicked");
            std::ptr::null()
        }
    }
}

/// Reports the C-ABI wrapper version (e.g. 1).
#[no_mangle]
pub unsafe extern "C" fn harper_abi_version() -> u32 {
    let result = std::panic::catch_unwind(|| HARPER_C_ABI_VERSION);
    match result {
        Ok(v) => v,
        Err(_) => {
            set_last_error("harper_abi_version panicked");
            0
        }
    }
}

/// Initialize a new checker instance with embedded curated dictionary.
#[no_mangle]
pub unsafe extern "C" fn harper_new(dialect: u32) -> Harper_Handle {
    let result = std::panic::catch_unwind(|| {
        clear_last_error();
        let core_dialect = match map_dialect(dialect) {
            Ok(d) => d,
            Err(e) => {
                set_last_error(e);
                return std::ptr::null_mut();
            }
        };

        let dictionary = FstDictionary::curated();
        let lint_group = LintGroup::new_curated(dictionary.clone(), core_dialect);

        let state = Box::new(Harper_State {
            lint_group,
            dictionary,
            dialect: core_dialect,
        });

        Box::into_raw(state)
    });

    match result {
        Ok(handle) => handle,
        Err(_) => {
            set_last_error("harper_new panicked");
            std::ptr::null_mut()
        }
    }
}

/// Destroy checker instance and reclaim dictionary resources.
#[no_mangle]
pub unsafe extern "C" fn harper_destroy(handle: Harper_Handle) {
    let _ = std::panic::catch_unwind(AssertUnwindSafe(|| {
        if !handle.is_null() {
            drop(Box::from_raw(handle));
        }
    }));
}

/// Run lint checks. Returns 0 on success, negative error code on failure.
#[no_mangle]
pub unsafe extern "C" fn harper_lint(
    handle: Harper_Handle,
    text: *const u8,
    len: usize,
    out: *mut Harper_Lint_Result,
) -> i32 {
    let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
        clear_last_error();

        if out.is_null() {
            set_last_error("Output pointer 'out' is null");
            return -2;
        }

        // Initialize output struct to clean zero state
        (*out).lints = std::ptr::null_mut();
        (*out).count = 0;

        if handle.is_null() {
            set_last_error("Checker handle is null");
            return -1;
        }

        if len == 0 {
            return 0;
        }

        if text.is_null() {
            set_last_error("Text pointer is null with non-zero length");
            return -3;
        }

        let bytes = std::slice::from_raw_parts(text, len);
        let text_str = match std::str::from_utf8(bytes) {
            Ok(s) => s,
            Err(e) => {
                set_last_error(format!("Text is not valid UTF-8: {e}"));
                return -4;
            }
        };

        let state = &mut *handle;
        let doc = Document::new(text_str, &PlainEnglish, state.dictionary.as_ref());

        let mut raw_lints = state.lint_group.lint(&doc);
        harper_core::remove_overlaps(&mut raw_lints);

        let mut c_lints = Vec::with_capacity(raw_lints.len());
        for lint in raw_lints {
            let Some((start, end)) = char_span_to_byte_span(text_str, lint.span) else {
                continue;
            };

            let Ok(start_u32) = u32::try_from(start) else {
                continue;
            };
            let Ok(end_u32) = u32::try_from(end) else {
                continue;
            };

            let (sug_kind, sug_cstr) = match lint.suggestions.first() {
                Some(Suggestion::ReplaceWith(chars)) => {
                    let s: String = chars.iter().collect();
                    let clean = s.replace('\0', " ");
                    let cstr = CString::new(clean).unwrap_or_default();
                    (HARPER_SUGGESTION_REPLACE, Some(cstr))
                }
                Some(Suggestion::InsertAfter(chars)) => {
                    let s: String = chars.iter().collect();
                    let clean = s.replace('\0', " ");
                    let cstr = CString::new(clean).unwrap_or_default();
                    (HARPER_SUGGESTION_INSERT_AFTER, Some(cstr))
                }
                Some(Suggestion::Remove) => (HARPER_SUGGESTION_REMOVE, None),
                None => (HARPER_SUGGESTION_REPLACE, None),
            };

            let clean_msg = lint.message.replace('\0', " ");
            let msg_cstr = CString::new(clean_msg).unwrap_or_else(|_| CString::new("").unwrap());

            let message_ptr = msg_cstr.into_raw() as *const c_char;
            let suggestion_ptr = sug_cstr
                .map(|c| c.into_raw() as *const c_char)
                .unwrap_or(std::ptr::null());

            c_lints.push(Harper_C_Lint {
                start: start_u32,
                end: end_u32,
                kind: map_lint_kind(lint.lint_kind),
                priority: lint.priority,
                suggestion_kind: sug_kind,
                _pad: 0,
                message: message_ptr,
                suggestion: suggestion_ptr,
            });
        }

        let count = c_lints.len();
        let lints_ptr = if count > 0 {
            let boxed = c_lints.into_boxed_slice();
            Box::into_raw(boxed) as *mut Harper_C_Lint
        } else {
            std::ptr::null_mut()
        };

        (*out).lints = lints_ptr;
        (*out).count = count;

        0
    }));

    match result {
        Ok(code) => code,
        Err(_) => {
            set_last_error("harper_lint panicked");
            -5
        }
    }
}

/// Free the entire result block, including all lint structs and strings.
#[no_mangle]
pub unsafe extern "C" fn harper_result_free(result: Harper_Lint_Result) {
    let _ = std::panic::catch_unwind(AssertUnwindSafe(|| {
        if result.lints.is_null() || result.count == 0 {
            return;
        }

        let slice = std::slice::from_raw_parts_mut(result.lints, result.count);
        for item in slice.iter_mut() {
            if !item.message.is_null() {
                drop(CString::from_raw(item.message as *mut c_char));
                item.message = std::ptr::null();
            }
            if !item.suggestion.is_null() {
                drop(CString::from_raw(item.suggestion as *mut c_char));
                item.suggestion = std::ptr::null();
            }
        }

        drop(Box::from_raw(std::ptr::slice_from_raw_parts_mut(
            result.lints,
            result.count,
        )));
    }));
}

/// Retrieve thread-local error details if an FFI call returns an error.
#[no_mangle]
pub unsafe extern "C" fn harper_last_error() -> *const c_char {
    let result = std::panic::catch_unwind(|| {
        LAST_ERROR.with(|cell| {
            cell.borrow()
                .as_ref()
                .map(|s| s.as_ptr())
                .unwrap_or(std::ptr::null())
        })
    });
    match result {
        Ok(ptr) => ptr,
        Err(_) => std::ptr::null(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CStr;

    #[test]
    fn test_version_endpoints() {
        unsafe {
            let ver_ptr = harper_version();
            assert!(!ver_ptr.is_null());
            let ver = CStr::from_ptr(ver_ptr).to_str().unwrap();
            assert_eq!(ver, "2.11.0");

            let abi_ver = harper_abi_version();
            assert_eq!(abi_ver, 1);
        }
    }

    #[test]
    fn test_char_span_to_byte_span_ascii() {
        let text = "hello world";
        assert_eq!(char_span_to_byte_span(text, Span::new(0, 5)), Some((0, 5)));
        assert_eq!(char_span_to_byte_span(text, Span::new(6, 11)), Some((6, 11)));
        assert_eq!(char_span_to_byte_span(text, Span::new(0, 0)), Some((0, 0)));
        assert_eq!(char_span_to_byte_span(text, Span::new(11, 11)), Some((11, 11)));
        assert_eq!(char_span_to_byte_span(text, Span::new(0, 12)), None);
    }

    #[test]
    fn test_char_span_to_byte_span_multibyte() {
        // 'Café': 'C'(1), 'a'(1), 'f'(1), 'é'(2 bytes: \xc3\xa9) -> 5 bytes, 4 chars
        // ' ' (1 byte)
        // 'is': 'i'(1), 's'(1) -> 2 bytes, 2 chars
        // ' ' (1 byte)
        // 'open': 'o'(1), 'p'(1), 'e'(1), 'n'(1) -> 4 bytes, 4 chars
        let text = "Café is open";
        // 'Café' is chars [0, 4) -> bytes [0, 5)
        let café_span = char_span_to_byte_span(text, Span::new(0, 4));
        assert_eq!(café_span, Some((0, 5)));
        assert_eq!(&text.as_bytes()[0..5], "Café".as_bytes());

        // 'is' is chars [5, 7) -> bytes [6, 8)
        let is_span = char_span_to_byte_span(text, Span::new(5, 7));
        assert_eq!(is_span, Some((6, 8)));
        assert_eq!(&text.as_bytes()[6..8], "is".as_bytes());

        // 'open' is chars [8, 12) -> bytes [9, 13)
        let open_span = char_span_to_byte_span(text, Span::new(8, 12));
        assert_eq!(open_span, Some((9, 13)));
        assert_eq!(&text.as_bytes()[9..13], "open".as_bytes());
    }

    #[test]
    fn test_char_span_to_byte_span_cjk_and_emoji() {
        // "你好, 🌍!"
        // '你' (3 bytes), '好' (3 bytes), ',' (1 byte), ' ' (1 byte), '🌍' (4 bytes), '!' (1 byte)
        let text = "你好, 🌍!";
        // '🌍' is char index 4..5 -> byte index 8..12
        let globe_span = char_span_to_byte_span(text, Span::new(4, 5));
        assert_eq!(globe_span, Some((8, 12)));
        assert_eq!(&text.as_bytes()[8..12], "🌍".as_bytes());
    }

    #[test]
    fn test_checker_lifecycle_and_spelling() {
        unsafe {
            let handle = harper_new(HARPER_DIALECT_AMERICAN);
            assert!(!handle.is_null());

            let text = "Thiss is bad";
            let mut result = Harper_Lint_Result {
                lints: std::ptr::null_mut(),
                count: 0,
            };

            let code = harper_lint(handle, text.as_ptr(), text.len(), &mut result);
            assert_eq!(code, 0);
            assert_eq!(result.count, 1);
            assert!(!result.lints.is_null());

            let lint = *result.lints;
            assert_eq!(lint.start, 0);
            assert_eq!(lint.end, 5);
            assert_eq!(&text.as_bytes()[lint.start as usize..lint.end as usize], b"Thiss");
            assert_eq!(lint.kind, HARPER_SPELLING);
            assert_eq!(lint.suggestion_kind, HARPER_SUGGESTION_REPLACE);
            assert!(!lint.message.is_null());
            assert!(!lint.suggestion.is_null());

            let sug = CStr::from_ptr(lint.suggestion).to_str().unwrap();
            assert_eq!(sug, "This");

            harper_result_free(result);
            harper_destroy(handle);
        }
    }

    #[test]
    fn test_multibyte_lint_offsets() {
        unsafe {
            let handle = harper_new(HARPER_DIALECT_AMERICAN);
            assert!(!handle.is_null());

            // "Café is bad, and thiss is wrong."
            // 'Café' is 5 bytes. 'thiss' starts at byte 18..23.
            let text = "Café is bad, and thiss is wrong.";
            let mut result = Harper_Lint_Result {
                lints: std::ptr::null_mut(),
                count: 0,
            };

            let code = harper_lint(handle, text.as_ptr(), text.len(), &mut result);
            assert_eq!(code, 0);
            assert!(result.count > 0);

            let lints = std::slice::from_raw_parts(result.lints, result.count);
            let spelling_lint = lints.iter().find(|l| l.kind == HARPER_SPELLING).expect("spelling lint found");

            assert_eq!(&text.as_bytes()[spelling_lint.start as usize..spelling_lint.end as usize], b"thiss");

            harper_result_free(result);
            harper_destroy(handle);
        }
    }

    #[test]
    fn test_thread_safety_send() {
        unsafe {
            let handle = harper_new(HARPER_DIALECT_AMERICAN);
            assert!(!handle.is_null());

            // Pass handle integer address across threads to simulate worker thread
            let handle_addr = handle as usize;
            let thread = std::thread::spawn(move || {
                let handle = handle_addr as Harper_Handle;
                let text = "Thiss is bad";
                let mut result = Harper_Lint_Result {
                    lints: std::ptr::null_mut(),
                    count: 0,
                };
                let code = harper_lint(handle, text.as_ptr(), text.len(), &mut result);
                assert_eq!(code, 0);
                assert_eq!(result.count, 1);
                harper_result_free(result);
                harper_destroy(handle);
            });

            thread.join().expect("Thread joined successfully");
        }
    }

    #[test]
    fn test_error_handling() {
        unsafe {
            // Invalid dialect
            let bad_handle = harper_new(999);
            assert!(bad_handle.is_null());
            let err_ptr = harper_last_error();
            assert!(!err_ptr.is_null());
            let err = CStr::from_ptr(err_ptr).to_str().unwrap();
            assert!(err.contains("Unsupported dialect"));

            // Null out pointer
            let handle = harper_new(HARPER_DIALECT_AMERICAN);
            let text = "Hello world";
            let code = harper_lint(handle, text.as_ptr(), text.len(), std::ptr::null_mut());
            assert_eq!(code, -2);
            let err = CStr::from_ptr(harper_last_error()).to_str().unwrap();
            assert!(err.contains("Output pointer 'out' is null"));

            // Null handle
            let mut result = Harper_Lint_Result {
                lints: std::ptr::null_mut(),
                count: 0,
            };
            let code = harper_lint(std::ptr::null_mut(), text.as_ptr(), text.len(), &mut result);
            assert_eq!(code, -1);
            let err = CStr::from_ptr(harper_last_error()).to_str().unwrap();
            assert!(err.contains("Checker handle is null"));

            // Null text with non-zero len
            let code = harper_lint(handle, std::ptr::null(), 10, &mut result);
            assert_eq!(code, -3);

            // Empty text (len 0) should succeed with 0 lints
            let code = harper_lint(handle, std::ptr::null(), 0, &mut result);
            assert_eq!(code, 0);
            assert_eq!(result.count, 0);
            assert!(result.lints.is_null());

            // Invalid UTF-8
            let bad_utf8 = [0xff, 0xfe, 0xfd];
            let code = harper_lint(handle, bad_utf8.as_ptr(), bad_utf8.len(), &mut result);
            assert_eq!(code, -4);
            let err = CStr::from_ptr(harper_last_error()).to_str().unwrap();
            assert!(err.contains("not valid UTF-8"));

            // Cleanup
            harper_destroy(handle);
            // Destroying null is safe
            harper_destroy(std::ptr::null_mut());
            // Freeing null result is safe
            harper_result_free(result);
        }
    }
}

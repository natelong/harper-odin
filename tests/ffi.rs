#![deny(warnings)]

use std::ffi::CStr;
use std::thread;

use harper_c::*;

#[test]
fn test_version_and_abi_metadata() {
    unsafe {
        let ver_ptr = harper_version();
        assert!(!ver_ptr.is_null(), "harper_version should not return null");
        let ver = CStr::from_ptr(ver_ptr)
            .to_str()
            .expect("harper_version should return valid UTF-8");
        assert_eq!(ver, "2.11.0", "harper_version must report pinned version 2.11.0");

        let abi_ver = harper_abi_version();
        assert_eq!(abi_ver, 1, "harper_abi_version must report ABI version 1");
    }
}

#[test]
fn test_lifecycle_clean_text() {
    unsafe {
        let handle = harper_new(HARPER_DIALECT_AMERICAN);
        assert!(!handle.is_null(), "harper_new must return valid handle");

        let clean_text = "This sentence contains no grammar or spelling errors.";
        let mut result = Harper_Lint_Result {
            lints: std::ptr::null_mut(),
            count: 0,
        };

        let ret = harper_lint(
            handle,
            clean_text.as_ptr(),
            clean_text.len(),
            &mut result,
        );
        assert_eq!(ret, 0, "harper_lint should return 0 for clean text");
        assert_eq!(result.count, 0, "Clean text should produce zero lints");
        assert!(result.lints.is_null(), "Lint pointer should be null when count is zero");

        // harper_result_free on empty result
        harper_result_free(result);

        // harper_destroy
        harper_destroy(handle);
    }
}

#[test]
fn test_spelling_detection_and_suggestion() {
    unsafe {
        let handle = harper_new(HARPER_DIALECT_AMERICAN);
        assert!(!handle.is_null(), "harper_new must return valid handle");

        let text = "Thiss is bad.";
        let mut result = Harper_Lint_Result {
            lints: std::ptr::null_mut(),
            count: 0,
        };

        let ret = harper_lint(handle, text.as_ptr(), text.len(), &mut result);
        assert_eq!(ret, 0, "harper_lint should return 0 on success");
        assert_eq!(result.count, 1, "Expected exactly 1 spelling lint");
        assert!(!result.lints.is_null(), "Lints pointer should not be null");

        let lint = *result.lints;
        assert_eq!(lint.kind, HARPER_SPELLING, "Lint kind must be HARPER_SPELLING");
        assert_eq!(
            lint.suggestion_kind, HARPER_SUGGESTION_REPLACE,
            "Suggestion kind must be REPLACE"
        );

        // Byte offsets must precisely match "Thiss" in text.as_bytes()
        assert_eq!(lint.start, 0, "Byte offset start should be 0");
        assert_eq!(lint.end, 5, "Byte offset end should be 5");
        let matched_bytes = &text.as_bytes()[lint.start as usize..lint.end as usize];
        assert_eq!(matched_bytes, b"Thiss");

        // Message verification
        assert!(!lint.message.is_null(), "Message pointer must not be null");
        let msg = CStr::from_ptr(lint.message)
            .to_str()
            .expect("Message must be valid UTF-8");
        assert!(!msg.is_empty(), "Message should not be empty");

        // Suggestion verification
        assert!(!lint.suggestion.is_null(), "Suggestion pointer must not be null");
        let sug = CStr::from_ptr(lint.suggestion)
            .to_str()
            .expect("Suggestion must be valid UTF-8");
        assert_eq!(sug, "This", "Suggestion for 'Thiss' should be 'This'");

        // Free result and destroy checker
        harper_result_free(result);
        harper_destroy(handle);
    }
}

#[test]
fn test_grammar_detection() {
    unsafe {
        let handle = harper_new(HARPER_DIALECT_AMERICAN);
        assert!(!handle.is_null(), "harper_new must return valid handle");

        // 1. HARPER_GRAMMAR: "did not went" -> "go"
        {
            let text = "We did not went there yesterday.";
            let mut result = Harper_Lint_Result {
                lints: std::ptr::null_mut(),
                count: 0,
            };

            let ret = harper_lint(handle, text.as_ptr(), text.len(), &mut result);
            assert_eq!(ret, 0, "harper_lint should return 0");
            assert!(result.count > 0, "Expected at least 1 lint for grammar mistake");

            let lints = std::slice::from_raw_parts(result.lints, result.count);
            let grammar_lint = lints
                .iter()
                .find(|l| l.kind == HARPER_GRAMMAR)
                .expect("Expected HARPER_GRAMMAR lint for 'did not went'");

            let matched_bytes = &text.as_bytes()[grammar_lint.start as usize..grammar_lint.end as usize];
            assert_eq!(matched_bytes, b"went", "Grammar lint should highlight 'went'");
            assert_eq!(grammar_lint.suggestion_kind, HARPER_SUGGESTION_REPLACE);

            assert!(!grammar_lint.suggestion.is_null());
            let sug = CStr::from_ptr(grammar_lint.suggestion).to_str().unwrap();
            assert_eq!(sug, "go", "Suggestion should replace 'went' with 'go'");

            harper_result_free(result);
        }

        // 2. HARPER_AGREEMENT: "They is happy." -> "are"
        {
            let text = "They is happy.";
            let mut result = Harper_Lint_Result {
                lints: std::ptr::null_mut(),
                count: 0,
            };

            let ret = harper_lint(handle, text.as_ptr(), text.len(), &mut result);
            assert_eq!(ret, 0, "harper_lint should return 0");
            assert!(result.count > 0, "Expected at least 1 lint for agreement error");

            let lints = std::slice::from_raw_parts(result.lints, result.count);
            let agreement_lint = lints
                .iter()
                .find(|l| l.kind == HARPER_AGREEMENT)
                .expect("Expected HARPER_AGREEMENT lint for 'They is'");

            let matched_bytes = &text.as_bytes()[agreement_lint.start as usize..agreement_lint.end as usize];
            assert_eq!(matched_bytes, b"is", "Agreement lint should highlight 'is'");

            assert!(!agreement_lint.suggestion.is_null());
            let sug = CStr::from_ptr(agreement_lint.suggestion).to_str().unwrap();
            assert_eq!(sug, "are", "Suggestion should replace 'is' with 'are'");

            harper_result_free(result);
        }

        // 3. HARPER_REPETITION: "I walked to the the park." -> "the"
        {
            let text = "I walked to the the park.";
            let mut result = Harper_Lint_Result {
                lints: std::ptr::null_mut(),
                count: 0,
            };

            let ret = harper_lint(handle, text.as_ptr(), text.len(), &mut result);
            assert_eq!(ret, 0, "harper_lint should return 0");
            assert!(result.count > 0, "Expected repetition lint");

            let lints = std::slice::from_raw_parts(result.lints, result.count);
            let rep_lint = lints
                .iter()
                .find(|l| l.kind == HARPER_REPETITION)
                .expect("Expected HARPER_REPETITION lint for 'the the'");

            let matched_bytes = &text.as_bytes()[rep_lint.start as usize..rep_lint.end as usize];
            assert_eq!(matched_bytes, b"the the");

            harper_result_free(result);
        }

        harper_destroy(handle);
    }
}

#[test]
fn test_multibyte_unicode_byte_offsets() {
    unsafe {
        let handle = harper_new(HARPER_DIALECT_AMERICAN);
        assert!(!handle.is_null(), "harper_new must return valid handle");

        // Case 1: 2-byte UTF-8 character 'é' preceding error
        // In "Café is open, but thiss is wrong.",
        // 'Café is open, but ' contains 18 Unicode chars, but 19 UTF-8 bytes (é is 2 bytes).
        // Therefore 'thiss' starts at byte 19, whereas char index would be 18.
        {
            let text = "Café is open, but thiss is wrong.";
            let mut result = Harper_Lint_Result {
                lints: std::ptr::null_mut(),
                count: 0,
            };

            let ret = harper_lint(handle, text.as_ptr(), text.len(), &mut result);
            assert_eq!(ret, 0);
            assert!(result.count > 0);

            let lints = std::slice::from_raw_parts(result.lints, result.count);
            let spelling_lint = lints
                .iter()
                .find(|l| l.kind == HARPER_SPELLING)
                .expect("Expected spelling lint for 'thiss'");

            assert_eq!(
                spelling_lint.start, 19,
                "Byte start must be 19 (UTF-8 byte offset, not char index 18)"
            );
            assert_eq!(
                spelling_lint.end, 24,
                "Byte end must be 24 (UTF-8 byte offset, not char index 23)"
            );

            let matched = &text.as_bytes()[spelling_lint.start as usize..spelling_lint.end as usize];
            assert_eq!(matched, b"thiss", "Byte range must match 'thiss' exactly");

            harper_result_free(result);
        }

        // Case 2: 4-byte emoji 🦀 preceding grammar error
        // 🦀 is 1 Unicode scalar (char), but 4 UTF-8 bytes: \xf0\x9f\xa6\x80.
        // In "🦀 We did not went to the café.",
        // '🦀 We did not ' has 13 Unicode characters, but 16 UTF-8 bytes.
        // Therefore 'went' byte range is [16, 20], whereas char index would be [13, 17].
        {
            let text = "🦀 We did not went to the café.";
            let mut result = Harper_Lint_Result {
                lints: std::ptr::null_mut(),
                count: 0,
            };

            let ret = harper_lint(handle, text.as_ptr(), text.len(), &mut result);
            assert_eq!(ret, 0);
            assert!(result.count > 0);

            let lints = std::slice::from_raw_parts(result.lints, result.count);
            let grammar_lint = lints
                .iter()
                .find(|l| l.kind == HARPER_GRAMMAR)
                .expect("Expected grammar lint for 'went'");

            assert_eq!(
                grammar_lint.start, 16,
                "Byte start must be 16 (UTF-8 byte offset, not char index 13)"
            );
            assert_eq!(
                grammar_lint.end, 20,
                "Byte end must be 20 (UTF-8 byte offset, not char index 17)"
            );

            let matched = &text.as_bytes()[grammar_lint.start as usize..grammar_lint.end as usize];
            assert_eq!(matched, b"went", "Byte range must match 'went' exactly");

            harper_result_free(result);
        }

        // Case 3: 3-byte CJK characters: "你好，世界！ Thiss is wrong."
        // 5 CJK characters + fullwidth punctuation = 18 bytes for 6 characters.
        // Plus 1 ASCII space = 19 bytes for 7 characters.
        // 'Thiss' is at char index 7..12, but byte index 19..24.
        {
            let text = "你好，世界！ Thiss is wrong.";
            let mut result = Harper_Lint_Result {
                lints: std::ptr::null_mut(),
                count: 0,
            };

            let ret = harper_lint(handle, text.as_ptr(), text.len(), &mut result);
            assert_eq!(ret, 0);
            assert!(result.count > 0);

            let lints = std::slice::from_raw_parts(result.lints, result.count);
            let spelling_lint = lints
                .iter()
                .find(|l| l.kind == HARPER_SPELLING)
                .expect("Expected spelling lint for 'Thiss'");

            assert_eq!(
                spelling_lint.start, 19,
                "Byte start must be 19 (UTF-8 byte offset, not char index 7)"
            );
            assert_eq!(
                spelling_lint.end, 24,
                "Byte end must be 24 (UTF-8 byte offset, not char index 12)"
            );

            let matched = &text.as_bytes()[spelling_lint.start as usize..spelling_lint.end as usize];
            assert_eq!(matched, b"Thiss", "Byte range must match 'Thiss' exactly");

            harper_result_free(result);
        }

        harper_destroy(handle);
    }
}

#[test]
fn test_error_handling_and_last_error() {
    unsafe {
        // 1. Invalid dialect ID
        let bad_handle = harper_new(9999);
        assert!(bad_handle.is_null(), "Invalid dialect ID must return null handle");
        let err_ptr = harper_last_error();
        assert!(!err_ptr.is_null(), "harper_last_error must return error description");
        let err_msg = CStr::from_ptr(err_ptr).to_str().unwrap();
        assert!(
            err_msg.contains("Unsupported dialect"),
            "Error message should mention unsupported dialect, got: {err_msg}"
        );

        // 2. Null checker handle
        let text = "Sample text";
        let mut result = Harper_Lint_Result {
            lints: std::ptr::null_mut(),
            count: 0,
        };
        let ret = harper_lint(std::ptr::null_mut(), text.as_ptr(), text.len(), &mut result);
        assert_eq!(ret, -1, "Null handle should return -1");
        let err_ptr = harper_last_error();
        assert!(!err_ptr.is_null());
        let err_msg = CStr::from_ptr(err_ptr).to_str().unwrap();
        assert!(
            err_msg.contains("Checker handle is null"),
            "Error message should mention null handle, got: {err_msg}"
        );

        // 3. Null out pointer
        let handle = harper_new(HARPER_DIALECT_AMERICAN);
        assert!(!handle.is_null());

        let ret = harper_lint(handle, text.as_ptr(), text.len(), std::ptr::null_mut());
        assert_eq!(ret, -2, "Null out pointer should return -2");
        let err_ptr = harper_last_error();
        assert!(!err_ptr.is_null());
        let err_msg = CStr::from_ptr(err_ptr).to_str().unwrap();
        assert!(
            err_msg.contains("Output pointer 'out' is null"),
            "Error message should mention null output pointer, got: {err_msg}"
        );

        // 4. Null text pointer with non-zero length
        let ret = harper_lint(handle, std::ptr::null(), 10, &mut result);
        assert_eq!(ret, -3, "Null text pointer with non-zero len should return -3");
        let err_ptr = harper_last_error();
        assert!(!err_ptr.is_null());
        let err_msg = CStr::from_ptr(err_ptr).to_str().unwrap();
        assert!(
            err_msg.contains("Text pointer is null"),
            "Error message should mention null text pointer, got: {err_msg}"
        );

        // 5. Invalid UTF-8 text
        let invalid_utf8 = [0xff, 0xfe, 0xfd];
        let ret = harper_lint(
            handle,
            invalid_utf8.as_ptr(),
            invalid_utf8.len(),
            &mut result,
        );
        assert_eq!(ret, -4, "Invalid UTF-8 should return -4");
        let err_ptr = harper_last_error();
        assert!(!err_ptr.is_null());
        let err_msg = CStr::from_ptr(err_ptr).to_str().unwrap();
        assert!(
            err_msg.contains("not valid UTF-8"),
            "Error message should mention UTF-8 error, got: {err_msg}"
        );

        // 6. Zero length text should succeed with 0 count and null lints
        let ret = harper_lint(handle, std::ptr::null(), 0, &mut result);
        assert_eq!(ret, 0, "Zero length text should succeed");
        assert_eq!(result.count, 0);
        assert!(result.lints.is_null());

        // 7. Re-freeing or freeing empty result should be safe
        harper_result_free(result);

        // 8. Safe destruction of valid handle and null handle
        harper_destroy(handle);
        harper_destroy(std::ptr::null_mut());
    }
}

#[test]
fn test_all_dialects_initialization() {
    let dialects = [
        (HARPER_DIALECT_AMERICAN, "American"),
        (HARPER_DIALECT_AUSTRALIAN, "Australian"),
        (HARPER_DIALECT_BRITISH, "British"),
        (HARPER_DIALECT_CANADIAN, "Canadian"),
    ];

    for (dialect_id, name) in dialects {
        unsafe {
            let handle = harper_new(dialect_id);
            assert!(
                !handle.is_null(),
                "harper_new should succeed for {name} dialect (id: {dialect_id})"
            );

            let text = "Testing dialect initialization.";
            let mut result = Harper_Lint_Result {
                lints: std::ptr::null_mut(),
                count: 0,
            };

            let ret = harper_lint(handle, text.as_ptr(), text.len(), &mut result);
            assert_eq!(ret, 0, "Linting should succeed for {name} dialect");

            harper_result_free(result);
            harper_destroy(handle);
        }
    }
}

#[test]
fn test_multiple_lints_in_single_text() {
    unsafe {
        let handle = harper_new(HARPER_DIALECT_AMERICAN);
        assert!(!handle.is_null());

        let text = "Thiss is bad and thatt is wrong.";
        let mut result = Harper_Lint_Result {
            lints: std::ptr::null_mut(),
            count: 0,
        };

        let ret = harper_lint(handle, text.as_ptr(), text.len(), &mut result);
        assert_eq!(ret, 0);
        assert!(result.count >= 2, "Expected at least 2 spelling errors");

        let lints = std::slice::from_raw_parts(result.lints, result.count);
        for lint in lints {
            assert!(
                lint.start < lint.end,
                "Start offset must precede end offset"
            );
            assert!(
                (lint.end as usize) <= text.len(),
                "End offset must not exceed text length"
            );

            // Byte slice validity
            let slice = &text.as_bytes()[lint.start as usize..lint.end as usize];
            assert!(
                slice == b"Thiss" || slice == b"thatt",
                "Expected either 'Thiss' or 'thatt', got {:?}",
                std::str::from_utf8(slice)
            );

            // Verify message is readable UTF-8
            assert!(!lint.message.is_null());
            let msg = CStr::from_ptr(lint.message).to_str();
            assert!(msg.is_ok());
        }

        harper_result_free(result);
        harper_destroy(handle);
    }
}

#[test]
fn test_multithreaded_concurrency() {
    let handles: Vec<_> = (0..4)
        .map(|_| unsafe { harper_new(HARPER_DIALECT_AMERICAN) })
        .collect();

    for h in &handles {
        assert!(!h.is_null());
    }

    let mut threads = Vec::new();
    for (i, &handle) in handles.iter().enumerate() {
        // Handle address is sent to worker thread
        let handle_addr = handle as usize;
        threads.push(thread::spawn(move || {
            let handle = handle_addr as Harper_Handle;
            let text = if i % 2 == 0 {
                "Thiss is a test of multithreading."
            } else {
                "Café is open, but thiss is also bad."
            };

            for _ in 0..5 {
                let mut result = Harper_Lint_Result {
                    lints: std::ptr::null_mut(),
                    count: 0,
                };
                unsafe {
                    let ret = harper_lint(handle, text.as_ptr(), text.len(), &mut result);
                    assert_eq!(ret, 0);
                    assert!(result.count > 0);
                    harper_result_free(result);
                }
            }

            unsafe {
                harper_destroy(handle);
            }
        }));
    }

    for t in threads {
        t.join().expect("Worker thread finished successfully");
    }
}

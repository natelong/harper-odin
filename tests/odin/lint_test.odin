package harper_test

// Standalone Odin test suite for the Harper grammar and spelling checker.
// Validates both low-level (harper_c) and high-level (harper) bindings against libharper_c.a.
// Tests cover FFI linkage, spelling/grammar detection, multibyte UTF-8 byte offset alignment,
// dialect configurations, error conditions, and zero-leak memory tracking.

import "core:c"
import "core:mem"
import "core:strings"
import "core:testing"
import harper "../../odin/harper"
import harper_c "../../odin/c"

// ---------------------------------------------------------------------------
// 1. Version and ABI Metadata Tests
// ---------------------------------------------------------------------------

@(test)
test_version_and_abi_metadata :: proc(t: ^testing.T) {
	// High-level API metadata
	ver := harper.version()
	testing.expect_value(t, ver, "2.11.0")

	abi_ver := harper.abi_version()
	testing.expect_value(t, abi_ver, 1)

	testing.expect_value(t, harper.C_ABI_VERSION, 1)

	// Low-level C-ABI metadata
	c_ver := harper_c.harper_version()
	testing.expect(t, c_ver != nil, "harper_c.harper_version should not return nil")
	testing.expect_value(t, string(c_ver), "2.11.0")

	c_abi_ver := harper_c.harper_abi_version()
	testing.expect_value(t, c_abi_ver, 1)

	// last_error should be empty initially
	err_msg := harper.last_error()
	testing.expect_value(t, err_msg, "")
}

// ---------------------------------------------------------------------------
// 2. Low-Level C-ABI Binding Tests (harper_c)
// ---------------------------------------------------------------------------

@(test)
test_low_level_foreign_c_abi :: proc(t: ^testing.T) {
	handle := harper_c.harper_new(.American)
	testing.expect(t, handle != nil, "harper_c.harper_new should return a valid non-nil handle")
	defer harper_c.harper_destroy(handle)

	// 1. Test clean text via low-level C-ABI
	clean_text := "This sentence contains no grammar or spelling errors."
	clean_res: harper_c.Lint_Result
	clean_code := harper_c.harper_lint(handle, raw_data(clean_text), c.size_t(len(clean_text)), &clean_res)
	testing.expect_value(t, clean_code, 0)
	testing.expect_value(t, clean_res.count, 0)
	testing.expect(t, clean_res.lints == nil, "Lints pointer should be nil when count is 0")
	harper_c.harper_result_free(clean_res)

	// 2. Test text with spelling error via low-level C-ABI
	bad_text := "Thiss is bad."
	bad_res: harper_c.Lint_Result
	bad_code := harper_c.harper_lint(handle, raw_data(bad_text), c.size_t(len(bad_text)), &bad_res)
	testing.expect_value(t, bad_code, 0)
	testing.expect_value(t, bad_res.count, 1)
	testing.expect(t, bad_res.lints != nil, "Lints pointer should not be nil when count > 0")

	if bad_res.count > 0 && bad_res.lints != nil {
		lint := bad_res.lints[0]
		testing.expect_value(t, lint.kind, harper_c.Lint_Kind.Spelling)
		testing.expect_value(t, lint.suggestion_kind, harper_c.Suggestion_Kind.Replace)
		testing.expect_value(t, lint.start, 0)
		testing.expect_value(t, lint.end, 5)

		// Byte slice verification
		matched := bad_text[lint.start:lint.end]
		testing.expect_value(t, matched, "Thiss")

		// Suggestion verification
		testing.expect(t, lint.suggestion != nil, "Suggestion should not be nil")
		testing.expect_value(t, string(lint.suggestion), "This")

		// Message verification
		testing.expect(t, lint.message != nil, "Message should not be nil")
		testing.expect(t, len(string(lint.message)) > 0, "Message should not be empty")
	}

	harper_c.harper_result_free(bad_res)
}

// ---------------------------------------------------------------------------
// 3. High-Level Clean Text Tests
// ---------------------------------------------------------------------------

@(test)
test_high_level_clean_text :: proc(t: ^testing.T) {
	handle, err := harper.harper_new_checked(.American)
	testing.expect_value(t, err, "")
	testing.expect(t, handle != nil, "handle should be non-nil")
	defer harper.destroy(handle)

	clean_text := "This sentence contains no grammar or spelling errors."
	anns, lint_err := harper.lint_text(handle, clean_text)
	testing.expect_value(t, lint_err, "")
	testing.expect_value(t, len(anns), 0)
	defer harper.destroy_annotations(anns)
}

// ---------------------------------------------------------------------------
// 4. Spelling Detection and Suggestion Tests
// ---------------------------------------------------------------------------

@(test)
test_spelling_detection_and_suggestions :: proc(t: ^testing.T) {
	handle, err := harper.harper_new_checked(.American)
	testing.expect_value(t, err, "")
	defer harper.destroy(handle)

	// Single spelling error
	text := "Thiss is bad."
	anns, lint_err := harper.lint_text(handle, text)
	testing.expect_value(t, lint_err, "")
	testing.expect_value(t, len(anns), 1)
	defer harper.destroy_annotations(anns)

	if len(anns) >= 1 {
		ann := anns[0]
		testing.expect_value(t, ann.kind, harper.Lint_Kind.Spelling)
		testing.expect_value(t, ann.suggestion_kind, harper.Suggestion_Kind.Replace)
		testing.expect_value(t, ann.start, 0)
		testing.expect_value(t, ann.end, 5)

		// Slice verification
		sliced := text[ann.start:ann.end]
		testing.expect_value(t, sliced, "Thiss")

		testing.expect_value(t, ann.suggestion, "This")
		testing.expect(t, len(ann.message) > 0, "Message should be non-empty")
	}

	// Multiple spelling errors in a single sentence
	multi_text := "Thiss is bad and thatt is wrong."
	multi_anns, multi_err := harper.lint_text(handle, multi_text)
	testing.expect_value(t, multi_err, "")
	testing.expect(t, len(multi_anns) >= 2, "Expected at least 2 spelling errors")
	defer harper.destroy_annotations(multi_anns)

	found_thiss := false
	found_thatt := false
	for ann in multi_anns {
		testing.expect(t, ann.start < ann.end, "Start offset must precede end offset")
		testing.expect(t, int(ann.end) <= len(multi_text), "End offset must be within text")

		sliced := multi_text[ann.start:ann.end]
		if sliced == "Thiss" {
			found_thiss = true
			testing.expect_value(t, ann.suggestion, "This")
		} else if sliced == "thatt" {
			found_thatt = true
			testing.expect_value(t, ann.suggestion, "that")
		}
	}
	testing.expect(t, found_thiss, "Expected lint for 'Thiss'")
	testing.expect(t, found_thatt, "Expected lint for 'thatt'")
}

// ---------------------------------------------------------------------------
// 5. Grammar, Agreement, and Repetition Tests
// ---------------------------------------------------------------------------

@(test)
test_grammar_and_agreement_detection :: proc(t: ^testing.T) {
	handle, err := harper.harper_new_checked(.American)
	testing.expect_value(t, err, "")
	defer harper.destroy(handle)

	// 1. HARPER_GRAMMAR: "did not went" -> "go"
	{
		text := "We did not went there yesterday."
		anns, lint_err := harper.lint_text(handle, text)
		testing.expect_value(t, lint_err, "")
		testing.expect(t, len(anns) > 0, "Expected at least 1 lint for grammar mistake")
		defer harper.destroy_annotations(anns)

		found_grammar := false
		for ann in anns {
			sliced := text[ann.start:ann.end]
			if ann.kind == .Grammar && sliced == "went" {
				found_grammar = true
				testing.expect_value(t, ann.suggestion, "go")
				testing.expect_value(t, ann.suggestion_kind, harper.Suggestion_Kind.Replace)
				break
			}
		}
		testing.expect(t, found_grammar, "Expected Grammar lint for 'went'")
	}

	// 2. HARPER_AGREEMENT: "They is happy." -> "are"
	{
		text := "They is happy."
		anns, lint_err := harper.lint_text(handle, text)
		testing.expect_value(t, lint_err, "")
		testing.expect(t, len(anns) > 0, "Expected agreement lint")
		defer harper.destroy_annotations(anns)

		found_agreement := false
		for ann in anns {
			if ann.kind == .Agreement {
				found_agreement = true
				sliced := text[ann.start:ann.end]
				testing.expect_value(t, sliced, "is")
				testing.expect_value(t, ann.suggestion, "are")
			}
		}
		testing.expect(t, found_agreement, "Expected Agreement lint for 'is'")
	}

	// 3. HARPER_REPETITION: "I walked to the the park." -> "the"
	{
		text := "I walked to the the park."
		anns, lint_err := harper.lint_text(handle, text)
		testing.expect_value(t, lint_err, "")
		testing.expect(t, len(anns) > 0, "Expected repetition lint")
		defer harper.destroy_annotations(anns)

		found_rep := false
		for ann in anns {
			if ann.kind == .Repetition {
				found_rep = true
				sliced := text[ann.start:ann.end]
				testing.expect_value(t, sliced, "the the")
			}
		}
		testing.expect(t, found_rep, "Expected Repetition lint for 'the the'")
	}
}

// ---------------------------------------------------------------------------
// 6. Multibyte UTF-8 Span Alignment Tests
// ---------------------------------------------------------------------------

@(test)
test_multibyte_utf8_spans :: proc(t: ^testing.T) {
	handle, err := harper.harper_new_checked(.American)
	testing.expect_value(t, err, "")
	defer harper.destroy(handle)

	// Case 1: 2-byte UTF-8 character 'é' (\xc3\xa9)
	// In "Café is open, but thiss is wrong.",
	// "Café is open, but " has 18 Unicode characters, but 19 UTF-8 bytes.
	// Harper must return byte offsets [19..24], NOT char indices [18..23].
	{
		text := "Café is open, but thiss is wrong."
		anns, lint_err := harper.lint_text(handle, text)
		testing.expect_value(t, lint_err, "")
		testing.expect(t, len(anns) > 0, "Expected spelling lint")
		defer harper.destroy_annotations(anns)

		found := false
		for ann in anns {
			if ann.kind == .Spelling {
				found = true
				testing.expect_value(t, ann.start, 19)
				testing.expect_value(t, ann.end, 24)

				// Verify byte slice without panic
				sliced := text[ann.start:ann.end]
				testing.expect_value(t, sliced, "thiss")
				testing.expect_value(t, ann.suggestion, "this")
			}
		}
		testing.expect(t, found, "Expected spelling lint for 'thiss'")
	}

	// Case 2: 4-byte UTF-8 emoji 🌍 (\xf0\x9f\x8c\x8d)
	// In "Hello 🌍 world, thiss is a test.",
	// "Hello 🌍 world, " has 15 characters, but 18 UTF-8 bytes (1 emoji = 4 bytes).
	// Harper must return byte offsets [18..23].
	{
		text := "Hello 🌍 world, thiss is a test."
		anns, lint_err := harper.lint_text(handle, text)
		testing.expect_value(t, lint_err, "")
		testing.expect(t, len(anns) > 0, "Expected spelling lint")
		defer harper.destroy_annotations(anns)

		found := false
		for ann in anns {
			if ann.kind == .Spelling {
				found = true
				testing.expect_value(t, ann.start, 18)
				testing.expect_value(t, ann.end, 23)

				// Verify byte slice without panic
				sliced := text[ann.start:ann.end]
				testing.expect_value(t, sliced, "thiss")
				testing.expect_value(t, ann.suggestion, "this")
			}
		}
		testing.expect(t, found, "Expected spelling lint for 'thiss'")
	}

	// Case 3: 4-byte UTF-8 emoji 🦀 preceding grammar error
	// In "🦀 We did not went to the café.",
	// "🦀 We did not " has 13 characters, but 16 UTF-8 bytes.
	// Grammar error 'went' byte range must be [16..20].
	{
		text := "🦀 We did not went to the café."
		anns, lint_err := harper.lint_text(handle, text)
		testing.expect_value(t, lint_err, "")
		testing.expect(t, len(anns) > 0, "Expected grammar lint")
		defer harper.destroy_annotations(anns)

		found := false
		for ann in anns {
			if ann.kind == .Grammar {
				found = true
				testing.expect_value(t, ann.start, 16)
				testing.expect_value(t, ann.end, 20)

				sliced := text[ann.start:ann.end]
				testing.expect_value(t, sliced, "went")
				testing.expect_value(t, ann.suggestion, "go")
			}
		}
		testing.expect(t, found, "Expected grammar lint for 'went'")
	}

	// Case 4: 3-byte CJK characters: "你好，世界！ Thiss is wrong."
	// 5 CJK characters + fullwidth punctuation = 18 bytes for 6 characters.
	// Plus 1 ASCII space = 19 bytes.
	// 'Thiss' byte range must be [19..24].
	{
		text := "你好，世界！ Thiss is wrong."
		anns, lint_err := harper.lint_text(handle, text)
		testing.expect_value(t, lint_err, "")
		testing.expect(t, len(anns) > 0, "Expected spelling lint")
		defer harper.destroy_annotations(anns)

		found := false
		for ann in anns {
			if ann.kind == .Spelling {
				found = true
				testing.expect_value(t, ann.start, 19)
				testing.expect_value(t, ann.end, 24)

				sliced := text[ann.start:ann.end]
				testing.expect_value(t, sliced, "Thiss")
				testing.expect_value(t, ann.suggestion, "This")
			}
		}
		testing.expect(t, found, "Expected spelling lint for 'Thiss'")
	}

	// Invariant check across all multibyte texts:
	// Verify every returned annotation slices cleanly without out-of-bounds or UTF-8 boundary panic.
	multibyte_samples := []string{
		"Café is open, but thiss is wrong.",
		"Hello 🌍 world, thiss is a test.",
		"🦀 We did not went to the café.",
		"你好，世界！ Thiss is wrong.",
		"Naïve résumé with som errorrs.",
	}

	for sample in multibyte_samples {
		anns, lint_err := harper.lint_text(handle, sample)
		testing.expect_value(t, lint_err, "")
		defer harper.destroy_annotations(anns)

		for ann in anns {
			testing.expect(t, ann.start < ann.end, "Start must be strictly less than end")
			testing.expect(t, int(ann.end) <= len(sample), "End must not exceed byte length")
			// Slice must never panic on valid UTF-8 byte offsets
			sliced := sample[ann.start:ann.end]
			testing.expect(t, len(sliced) > 0, "Sliced text must not be empty")
		}
	}
}

// ---------------------------------------------------------------------------
// 7. Dialect Handling Tests
// ---------------------------------------------------------------------------

@(test)
test_dialect_handling :: proc(t: ^testing.T) {
	// Compare American vs British spelling rules on the word "colour"
	handle_us, err_us := harper.harper_new_checked(.American)
	testing.expect_value(t, err_us, "")
	defer harper.destroy(handle_us)

	handle_uk, err_uk := harper.harper_new_checked(.British)
	testing.expect_value(t, err_uk, "")
	defer harper.destroy(handle_uk)

	text := "The colour of the sky is blue."

	// In American dialect, "colour" is flagged as a spelling mistake, suggesting "color"
	anns_us, err_lint_us := harper.lint_text(handle_us, text)
	testing.expect_value(t, err_lint_us, "")
	defer harper.destroy_annotations(anns_us)

	found_us_colour := false
	for ann in anns_us {
		if ann.kind == .Spelling && text[ann.start:ann.end] == "colour" {
			found_us_colour = true
			testing.expect_value(t, ann.suggestion, "color")
		}
	}
	testing.expect(t, found_us_colour, "American dialect must flag 'colour' and suggest 'color'")

	// In British dialect, "colour" is correct and must NOT be flagged as a spelling mistake
	anns_uk, err_lint_uk := harper.lint_text(handle_uk, text)
	testing.expect_value(t, err_lint_uk, "")
	defer harper.destroy_annotations(anns_uk)

	found_uk_colour := false
	for ann in anns_uk {
		if ann.kind == .Spelling && text[ann.start:ann.end] == "colour" {
			found_uk_colour = true
		}
	}
	testing.expect(t, !found_uk_colour, "British dialect must accept 'colour' without spelling error")

	// Verify all supported dialects can be initialized and execute linting
	dialects := []harper.Dialect{
		.American,
		.Australian,
		.British,
		.Canadian,
	}

	for d in dialects {
		h, init_err := harper.harper_new_checked(d)
		testing.expect_value(t, init_err, "")
		testing.expect(t, h != nil, "Handle must not be nil for valid dialect")

		sample_text := "Testing dialect initialization and basic linting."
		sample_anns, sample_err := harper.lint_text(h, sample_text)
		testing.expect_value(t, sample_err, "")
		harper.destroy_annotations(sample_anns)
		harper.destroy(h)
	}
}

// ---------------------------------------------------------------------------
// 8. Error Handling and Edge Cases
// ---------------------------------------------------------------------------

@(test)
test_error_handling_and_edge_cases :: proc(t: ^testing.T) {
	// 1. Invalid dialect ID to harper_new_checked
	bad_handle, bad_err := harper.harper_new_checked(harper.Dialect(9999))
	testing.expect(t, bad_handle == nil, "Invalid dialect must return nil handle")
	testing.expect(t, len(bad_err) > 0, "Invalid dialect must return descriptive error message")
	testing.expect(t, strings.contains(bad_err, "Unsupported dialect"), "Error must mention unsupported dialect")

	// 2. Empty string text linting
	handle, err := harper.harper_new_checked(.American)
	testing.expect_value(t, err, "")
	defer harper.destroy(handle)

	empty_anns, empty_err := harper.lint_text(handle, "")
	testing.expect_value(t, empty_err, "")
	testing.expect_value(t, len(empty_anns), 0)
	harper.destroy_annotations(empty_anns)

	// 3. Low-level: Null checker handle
	res: harper_c.Lint_Result
	sample := "Hello world"
	ret_null_handle := harper_c.harper_lint(nil, raw_data(sample), c.size_t(len(sample)), &res)
	testing.expect_value(t, ret_null_handle, -1)
	err_msg := harper.last_error()
	testing.expect(t, strings.contains(err_msg, "Checker handle is null"), "Expected 'Checker handle is null' error")

	// 4. Low-level: Null out result pointer
	ret_null_out := harper_c.harper_lint(handle, raw_data(sample), c.size_t(len(sample)), nil)
	testing.expect_value(t, ret_null_out, -2)
	err_out := harper.last_error()
	testing.expect(t, strings.contains(err_out, "Output pointer 'out' is null"), "Expected 'Output pointer' error")

	// 5. Low-level: Null text pointer with non-zero length
	ret_null_text := harper_c.harper_lint(handle, nil, 10, &res)
	testing.expect_value(t, ret_null_text, -3)
	err_text := harper.last_error()
	testing.expect(t, strings.contains(err_text, "Text pointer is null"), "Expected 'Text pointer is null' error")

	// 6. Low-level: Invalid UTF-8 text bytes
	invalid_utf8 := [3]u8{0xff, 0xfe, 0xfd}
	ret_invalid_utf8 := harper_c.harper_lint(handle, raw_data(invalid_utf8[:]), 3, &res)
	testing.expect_value(t, ret_invalid_utf8, -4)
	err_utf8 := harper.last_error()
	testing.expect(t, strings.contains(err_utf8, "valid UTF-8"), "Expected UTF-8 error")

	// 7. Safe destruction of nil handles
	harper.destroy(nil)
	harper_c.harper_destroy(nil)
}

// ---------------------------------------------------------------------------
// 9. Memory Leak Detection (core:mem Tracking Allocator)
// ---------------------------------------------------------------------------

@(test)
test_memory_leak_detection :: proc(t: ^testing.T) {
	// Initialize tracking allocator wrapping context.allocator
	track: mem.Tracking_Allocator
	mem.tracking_allocator_init(&track, context.allocator)
	defer mem.tracking_allocator_destroy(&track)

	// Route all allocations in this test through tracking allocator
	context.allocator = mem.tracking_allocator(&track)

	// Exercise full lifecycle across multiple rounds and diverse text inputs
	rounds := 20
	for _ in 0..<rounds {
		handle, err := harper.harper_new_checked(.American)
		testing.expect_value(t, err, "")
		testing.expect(t, handle != nil, "handle must be non-nil")

		test_texts := []string{
			"This sentence contains no grammar or spelling errors.",
			"Thiss is bad and thatt is wrong.",
			"We did not went there yesterday.",
			"They is happy.",
			"Café is open, but thiss is wrong.",
			"Hello 🌍 world, thiss is a test.",
			"你好，世界！ Thiss is wrong.",
			"",
		}

		for text in test_texts {
			anns, lint_err := harper.lint_text(handle, text)
			testing.expect_value(t, lint_err, "")

			// Validate slicing
			for ann in anns {
				testing.expect(t, ann.start < ann.end, "Start must precede end")
				testing.expect(t, int(ann.end) <= len(text), "End within bounds")
				sliced := text[ann.start:ann.end]
				testing.expect(t, len(sliced) > 0, "Slice not empty")
			}

			harper.destroy_annotations(anns)
		}

		harper.destroy(handle)
	}

	// Assert zero memory leaks tracked
	if len(track.allocation_map) > 0 {
		for _, leak in track.allocation_map {
			testing.expectf(t, false, "Memory leak: %v bytes at %v\n", leak.size, leak.location)
		}
	}
	testing.expect_value(t, len(track.allocation_map), 0)
	testing.expect_value(t, track.current_memory_allocated, 0)
	testing.expect_value(t, len(track.bad_free_array), 0)
}

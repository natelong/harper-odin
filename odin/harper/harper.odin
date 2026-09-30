package harper

// High-level idiomatic Odin wrapper for the Harper grammar and spelling checker.
// Wraps the low-level C-ABI bindings from package harper_c.

import "core:c"
import "core:mem"
import "core:strings"
import harper_c "../c"

// Re-export common types from harper_c for convenience
Dialect         :: harper_c.Dialect
Lint_Kind       :: harper_c.Lint_Kind
Suggestion_Kind :: harper_c.Suggestion_Kind
Handle          :: harper_c.Handle
C_ABI_VERSION   :: harper_c.C_ABI_VERSION

// High-level representation of a grammar or spelling lint annotation.
// Byte offsets `start` and `end` index directly into the checked UTF-8 text string.
Annotation :: struct {
	start:           u32,
	end:             u32,
	kind:            Lint_Kind,
	priority:        u8,
	suggestion_kind: Suggestion_Kind,
	message:         string,
	suggestion:      string,
}

// Reports the pinned harper-core version string (e.g. "2.11.0").
version :: proc() -> string {
	ver := harper_c.harper_version()
	return string(ver) if ver != nil else ""
}

// Reports the C-ABI wrapper version (e.g. 1).
abi_version :: proc() -> u32 {
	return u32(harper_c.harper_abi_version())
}

// Retrieves the thread-local error details from the last failed FFI call, if any.
last_error :: proc() -> string {
	err := harper_c.harper_last_error()
	return string(err) if err != nil else ""
}

// Initializes a new Harper checker instance configured for the specified dialect.
// Checks for initialization errors and returns (handle, "") on success, or (nil, error_message)
// if initialization failed.
harper_new_checked :: proc(dialect: Dialect = .American) -> (Handle, string) {
	handle := harper_c.harper_new(dialect)
	if handle == nil {
		err_cstr := harper_c.harper_last_error()
		if err_cstr != nil {
			return nil, string(err_cstr)
		}
		return nil, "Failed to initialize Harper checker instance"
	}
	return handle, ""
}

// Lints the provided UTF-8 text string using the specified Harper checker handle.
// Returns an owned slice of Annotations with cloned strings and an empty error string on success.
// If an error occurs, returns (nil, error_message).
// The caller is responsible for freeing the returned annotations slice and its strings
// using `destroy_annotations(annotations, allocator)`.
lint_text :: proc(handle: Handle, text: string, allocator := context.allocator) -> ([]Annotation, string) {
	result: harper_c.Lint_Result
	defer harper_c.harper_result_free(result)

	text_ptr: [^]u8 = nil
	if len(text) > 0 {
		text_ptr = raw_data(text)
	}

	code := harper_c.harper_lint(handle, text_ptr, c.size_t(len(text)), &result)
	if code != 0 {
		err_cstr := harper_c.harper_last_error()
		if err_cstr != nil {
			return nil, string(err_cstr)
		}
		return nil, "harper_lint failed with an unexpected error"
	}

	if result.count == 0 || result.lints == nil {
		return make([]Annotation, 0, allocator), ""
	}

	annotations := make([]Annotation, result.count, allocator)
	c_lints := result.lints[:int(result.count)]

	for i in 0..<int(result.count) {
		cl := c_lints[i]

		msg: string
		if cl.message != nil {
			msg = strings.clone_from_cstring(cl.message, allocator)
		}

		sug: string
		if cl.suggestion != nil {
			sug = strings.clone_from_cstring(cl.suggestion, allocator)
		}

		annotations[i] = Annotation{
			start           = u32(cl.start),
			end             = u32(cl.end),
			kind            = cl.kind,
			priority        = u8(cl.priority),
			suggestion_kind = cl.suggestion_kind,
			message         = msg,
			suggestion      = sug,
		}
	}

	return annotations, ""
}

// Safely frees memory allocated for all string fields in each Annotation,
// and frees the annotations slice itself.
destroy_annotations :: proc(annotations: []Annotation, allocator := context.allocator) {
	for ann in annotations {
		delete(ann.message, allocator)
		delete(ann.suggestion, allocator)
	}
	delete(annotations, allocator)
}

// Destroys the Harper checker instance and frees all associated dictionary and state resources.
destroy :: proc(handle: Handle) {
	harper_c.harper_destroy(handle)
}

// Empty entry point procedure allowing `odin check odin/harper` to succeed directly.
@(private)
main :: proc() {}

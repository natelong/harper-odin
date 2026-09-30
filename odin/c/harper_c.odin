package harper_c

import "core:c"

when ODIN_OS == .Darwin {
	foreign import harper_lib { "system:libharper_c.a", "system:pthread" }
} else {
	foreign import harper_lib { "system:libharper_c.a", "system:pthread", "system:dl", "system:m" }
}

C_ABI_VERSION :: 1

Dialect :: enum c.uint32_t {
	American   = 0,
	Australian = 1,
	British    = 2,
	Canadian   = 3,
}

Lint_Kind :: enum c.uint8_t {
	Agreement        = 0,
	Boundary_Error   = 1,
	Capitalization   = 2,
	Eggcorn          = 3,
	Enhancement      = 4,
	Formatting       = 5,
	Grammar          = 6,
	Malapropism      = 7,
	Miscellaneous    = 8,
	Nonstandard      = 9,
	Punctuation      = 10,
	Readability      = 11,
	Redundancy       = 12,
	Regionalism      = 13,
	Repetition       = 14,
	Spelling         = 15,
	Style            = 16,
	Typo             = 17,
	Usage            = 18,
	Word_Choice      = 19,
	Word_Order       = 20,
}

Suggestion_Kind :: enum c.uint8_t {
	Replace      = 0,
	Insert_After = 1,
	Remove       = 2,
}

C_Lint :: struct {
	start:           c.uint32_t,
	end:             c.uint32_t,
	kind:            Lint_Kind,
	priority:        c.uint8_t,
	suggestion_kind: Suggestion_Kind,
	_pad:            c.uint8_t,
	message:         cstring,
	suggestion:      cstring,
}

Lint_Result :: struct {
	lints: [^]C_Lint,
	count: c.size_t,
}

Handle :: distinct rawptr

@(default_calling_convention="c")
foreign harper_lib {
	harper_version :: proc() -> cstring ---
	harper_abi_version :: proc() -> c.uint32_t ---
	harper_new :: proc(dialect: Dialect) -> Handle ---
	harper_destroy :: proc(handle: Handle) ---
	harper_lint :: proc(handle: Handle, text: [^]u8, len: c.size_t, out: ^Lint_Result) -> c.int32_t ---
	harper_result_free :: proc(result: Lint_Result) ---
	harper_last_error :: proc() -> cstring ---
}

# Specification: Standalone `harper-odin` Library & Odin Package

This document is a self-contained, turnkey specification for building the external **`harper-odin`** repository from scratch. It is designed to be provided directly to an autonomous agent or developer tasked with implementing the standalone library.

---

## 1. Project Overview & Mission

**Repository Name**: `harper-odin`  
**Purpose**: Provide a production-grade, precompiled C-ABI static and dynamic library and native Odin package wrapping `harper-core` (v2.11.0) for offline, deterministic grammar and spelling linting with zero panics and UTF-8 byte offset guarantees.

### Core Objectives
1. **Zero Rust Toolchain Requirement for Consumers**: Produce precompiled release archives (`libharper_c.a` and `libharper_c.dylib`) so consumers (such as the Maxwell text editor) require only an Odin compiler.
2. **Strict FFI Invariants**:
   - **Byte offsets**: Convert Harper's character (scalar) indices to UTF-8 byte offsets in Rust before returning across FFI.
   - **Whole-result ownership**: Allocate lints and strings in a single result arena, reclaimed with a single `harper_result_free`.
   - **Zero panics**: Wrap all C entry points in `std::panic::catch_unwind`, funneling error details to thread-local storage.
   - **Thread-safe**: Ensure `Harper_Handle` is `Send`, allowing lint worker threads off the main UI thread.
3. **Idiomatic Odin Package**: Provide low-level foreign bindings (`harper/c`) and high-level abstractions (`package harper`).
4. **Multi-Platform CI Releases**: GitHub Actions pipeline generating release tarballs for macOS (arm64, x86_64) and Linux (x86_64) with SHA-256 checksums.

---

## 2. Target Repository Layout

```
harper-odin/
├── Cargo.toml                      # Rust crate configuration (staticlib and cdylib)
├── src/
│   └── lib.rs                      # extern "C" implementation, catch_unwind, byte conversions
├── include/
│   └── harper_c.h                  # Canonical C-ABI header file
├── tests/
│   └── ffi.rs                      # Rust integration tests for the C-ABI
├── odin/
│   ├── harper/
│   │   └── harper.odin             # High-level Odin wrappers (Annotation, lint_text, destroy)
│   └── c/
│       └── harper_c.odin           # Low-level foreign C function & struct declarations
├── tests/
│   └── odin/
│       └── lint_test.odin          # Standalone Odin test suite
├── .github/
│   └── workflows/
│       ├── test.yml                # CI test validation (cargo test + odin test)
│       └── release.yml             # Automated multi-platform release packager
├── LICENSE                         # Apache-2.0
└── README.md                       # Documentation & integration instructions
```

---

## 3. Rust Crate Configuration (`Cargo.toml`)

```toml
[package]
name = "harper_c"
version = "0.1.0"
edition = "2021"
authors = ["Maxwell Authors"]
license = "Apache-2.0"
description = "C-ABI and Odin bindings for the Harper grammar checker"

[lib]
crate-type = ["staticlib", "cdylib", "rlib"]

[dependencies]
harper-core = { version = "2.11.0", default-features = false, features = ["concurrent", "std"] }

[profile.release]
opt-level = 3
lto = "fat"
codegen-units = 1
panic = "unwind"      # Required: must NEVER be abort, so catch_unwind works across FFI
strip = true
```

---

## 4. The C-ABI Surface (`include/harper_c.h`)

```c
#ifndef HARPER_C_H
#define HARPER_C_H

#include <stdint.h>
#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

#define HARPER_C_ABI_VERSION 1

/* Reports the pinned harper-core version (e.g. "2.11.0"). */
const char *harper_version(void);

/* Reports the C-ABI wrapper version (e.g. 1). */
uint32_t harper_abi_version(void);

/* Supported language dialects. */
enum Harper_Dialect {
    HARPER_DIALECT_AMERICAN   = 0,
    HARPER_DIALECT_AUSTRALIAN = 1,
    HARPER_DIALECT_BRITISH    = 2,
    HARPER_DIALECT_CANADIAN   = 3,
};

/* Lint kinds matching harper-core 2.11.0. */
enum Harper_Lint_Kind {
    HARPER_AGREEMENT        = 0,
    HARPER_BOUNDARY_ERROR   = 1,
    HARPER_CAPITALIZATION   = 2,
    HARPER_EGGCORN          = 3,
    HARPER_ENHANCEMENT      = 4,
    HARPER_FORMATTING       = 5,
    HARPER_GRAMMAR          = 6,
    HARPER_MALAPROPISM      = 7,
    HARPER_MISCELLANEOUS    = 8,
    HARPER_NONSTANDARD      = 9,
    HARPER_PUNCTUATION      = 10,
    HARPER_READABILITY      = 11,
    HARPER_REDUNDANCY       = 12,
    HARPER_REGIONALISM      = 13,
    HARPER_REPETITION       = 14,
    HARPER_SPELLING         = 15,
    HARPER_STYLE            = 16,
    HARPER_TYPO             = 17,
    HARPER_USAGE            = 18,
    HARPER_WORD_CHOICE      = 19,
    HARPER_WORD_ORDER       = 20,
};

/* Suggestion actions. */
enum Harper_Suggestion_Kind {
    HARPER_SUGGESTION_REPLACE      = 0,
    HARPER_SUGGESTION_INSERT_AFTER = 1,
    HARPER_SUGGESTION_REMOVE       = 2,
};

/* Wire representation of an individual lint. Offsets are UTF-8 byte indices! */
typedef struct Harper_C_Lint {
    uint32_t start;          /* Byte offset into checked text (char boundary) */
    uint32_t end;            /* Byte offset, exclusive */
    uint8_t  kind;           /* Harper_Lint_Kind */
    uint8_t  priority;       /* Lower = higher priority */
    uint8_t  suggestion_kind;/* Harper_Suggestion_Kind */
    uint8_t  _pad;
    const char *message;     /* NUL-terminated UTF-8, owned by result block */
    const char *suggestion;  /* NUL-terminated UTF-8 replacement, or NULL */
} Harper_C_Lint;

/* Whole-result allocation block. */
typedef struct Harper_Lint_Result {
    Harper_C_Lint *lints;
    size_t count;
} Harper_Lint_Result;

/* Opaque instance handle */
typedef struct Harper_State Harper_Handle;

/* Initialize a new checker instance with embedded curated dictionary. */
Harper_Handle harper_new(uint32_t dialect);

/* Destroy checker instance and reclaim dictionary resources. */
void harper_destroy(Harper_Handle handle);

/* Run lint checks. Returns 0 on success, negative error code on failure. */
int32_t harper_lint(Harper_Handle handle, const uint8_t *text, size_t len,
                    Harper_Lint_Result *out);

/* Free the entire result block, including all lint structs and strings. */
void harper_result_free(Harper_Lint_Result result);

/* Retrieve thread-local error details if an FFI call returns an error. */
const char *harper_last_error(void);

#ifdef __cplusplus
}
#endif

#endif /* HARPER_C_H */
```

---

## 5. Critical Implementation Invariants (`src/lib.rs`)

### 5.1 Character Index to Byte Offset Conversion
Harper returns `Span` in Unicode scalar values (`chars`). The wrapper must convert to UTF-8 byte offsets:
```rust
fn char_span_to_byte_span(text: &str, span: harper_core::Span) -> Option<(usize, usize)> {
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
    if span.end == char_idx && byte_end.is_none() {
        byte_end = Some(text.len());
    }

    match (byte_start, byte_end) {
        (Some(s), Some(e)) if s <= e && e <= text.len() => Some((s, e)),
        _ => None,
    }
}
```

### 5.2 Panic Safety
Every `extern "C"` function must use `std::panic::catch_unwind`:
```rust
#[no_mangle]
pub unsafe extern "C" fn harper_lint(
    handle: Harper_Handle,
    text: *const u8,
    len: usize,
    out: *mut Harper_Lint_Result,
) -> i32 {
    let result = std::panic::catch_unwind(|| {
        // Implementation logic
        0
    });
    match result {
        Ok(code) => code,
        Err(err) => {
            set_last_error("harper_lint panicked");
            -1
        }
    }
}
```

---

## 6. Native Odin Package

### 6.1 Low-Level Foreign Declarations (`odin/c/harper_c.odin`)
```odin
package harper_c

import "core:c"

when ODIN_OS == .Darwin {
    foreign import harper_lib { "system:libharper_c.a", "system:pthread" }
} else {
    foreign import harper_lib { "system:libharper_c.a", "system:pthread", "system:dl", "system:m" }
}

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
```

### 6.2 High-Level Odin API (`odin/harper/harper.odin`)
Exposes:
- `Annotation` struct with byte offsets, `message`, `suggestion`, `kind`, and `priority`.
- `harper_new_checked() -> (Handle, string)`
- `lint_text(handle: Handle, text: string) -> ([]Annotation, string)`
- `destroy_annotations(annotations: []Annotation)`

---

## 7. Standalone Test Verification

1. **Rust FFI Tests (`tests/ffi.rs`)**:
   - `cargo test` executes integration tests exercising `harper_new`, `harper_lint`, `harper_result_free`, and `harper_destroy`.
   - Tests spelling errors (`"Thiss is bad"`), grammar errors, and multibyte Unicode character boundaries (`"Café is open"`).
2. **Odin Tests (`tests/odin/`)**:
   - `odin test tests/odin/ -define:ODIN_TEST_THREADS=1`
   - Verifies zero memory leaks and exact byte offset alignment.

---

## 8. CI Release Pipeline (`.github/workflows/release.yml`)

Matrix builds for tagged releases (`v*`):
- `macos-14` (Apple Silicon `aarch64-apple-darwin`)
- `macos-13` (Intel `x86_64-apple-darwin`)
- `ubuntu-latest` (Linux `x86_64-unknown-linux-gnu`)

Generates `harper-odin-{TARGET}-v{VERSION}.tar.gz` containing:
- `lib/libharper_c.a` and `lib/libharper_c.dylib` (or `.so`)
- `include/harper_c.h`
- `odin/` package sources
- `LICENSE` and `README.md`
- `checksums.txt` with SHA-256 hashes

---

## 9. Definition of Done for `harper-odin` Agent

The implementation is complete when:
- [ ] `cargo test` passes 100% of Rust tests with zero warnings.
- [ ] `cargo build --release` produces `libharper_c.a` without errors.
- [ ] `odin test tests/odin/` passes cleanly using the compiled static library.
- [ ] Multibyte UTF-8 span test verifies byte offset accuracy.
- [ ] GitHub Actions release workflow is verified.

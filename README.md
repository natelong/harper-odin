# harper-odin

[![Test](https://github.com/natelong/harper-odin/actions/workflows/test.yml/badge.svg)](https://github.com/natelong/harper-odin/actions/workflows/test.yml)
[![Release](https://github.com/natelong/harper-odin/actions/workflows/release.yml/badge.svg)](https://github.com/natelong/harper-odin/actions/workflows/release.yml)
[![License](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

Production-grade C-ABI static and dynamic library and native Odin package wrapping [Harper](https://github.com/Automattic/harper) (`harper-core` v2.11.0) for offline, deterministic grammar and spelling linting with zero panics and UTF-8 byte offset guarantees.

---

## Table of Contents

- [Overview & Purpose](#overview--purpose)
- [Design Goals & Architecture](#design-goals--architecture)
  - [Zero Rust Toolchain Requirement](#zero-rust-toolchain-requirement)
  - [Strict FFI Invariants](#strict-ffi-invariants)
- [Repository Layout](#repository-layout)
- [C-ABI API Reference](#c-abi-api-reference)
  - [Types & Enums](#types--enums)
  - [Functions](#functions)
  - [C Usage Example](#c-usage-example)
- [Odin Package Usage](#odin-package-usage)
  - [High-Level API (`package harper`)](#high-level-api-package-harper)
  - [Low-Level API (`package harper_c`)](#low-level-api-package-harper_c)
  - [Configuring the Library Path](#configuring-the-library-path)
- [Local Development & Testing](#local-development--testing)
  - [Prerequisites](#prerequisites)
  - [Building from Source](#building-from-source)
  - [Running the Test Suites](#running-the-test-suites)
- [Precompiled Binary Releases & Integration](#precompiled-binary-releases--integration)
  - [Release Artifacts](#release-artifacts)
  - [Integrating into Downstream Projects](#integrating-into-downstream-projects)
- [License](#license)

---

## Overview & Purpose

**harper-odin** exposes the high-performance offline grammar and spell checking engine of `harper-core` to Odin programs and C-ABI consumers.

Modern software editors, word processors, and command-line tools require responsive grammar and spell checking that functions fully offline, respects user privacy, and does not require complex runtime dependencies. While `harper-core` is implemented in Rust, many native applications are built in languages like Odin or C/C++.

`harper-odin` bridges this gap by providing:
1. A clean, panic-safe C-ABI static (`libharper_c.a`) and dynamic (`libharper_c.dylib` / `.so`) library.
2. A canonical C header (`include/harper_c.h`).
3. An idiomatic, native Odin package (`odin/harper`) and low-level foreign bindings (`odin/c`).
4. Precompiled multi-platform binary releases with checksums, eliminating the need for consumers to install Rust or Cargo.

---

## Design Goals & Architecture

### Zero Rust Toolchain Requirement

Downstream consumers should not be forced to install the Rust toolchain, configure Cargo, or compile Rust source dependencies during their build processes.

- Every tagged release publishes precompiled binary archives containing static and dynamic libraries for supported platforms (macOS Apple Silicon, macOS Intel, and Linux x86_64).
- Odin consumers drop the `odin/` package into their project (or link via collection) and point to the precompiled `libharper_c.a`.
- The build process for consumers remains 100% pure Odin.

### Strict FFI Invariants

Crossing the language barrier between Rust and Odin/C introduces potential pitfalls around string indices, memory management, and error handling. `harper-odin` enforces four strict architectural invariants:

1. **UTF-8 Byte Offsets**:
   `harper-core` calculates spans using Unicode scalar values (`char` counts). However, Odin strings and C strings index by UTF-8 bytes. `harper-odin` translates all character spans to valid UTF-8 byte offsets inside Rust before passing them over FFI. Downstream consumers can directly slice strings (e.g., `text[ann.start:ann.end]`) without panics, multibyte boundary splitting, or character re-scanning.
2. **Whole-Result Arena Ownership**:
   All lint structures and their associated message and suggestion strings are allocated together in a single result arena managed by the library. The consumer reclaims the entire result block with a single call to `harper_result_free`, preventing per-string allocation overhead and memory leaks.
3. **Zero Panics Across FFI**:
   Unwinding across an `extern "C"` boundary is undefined behavior in C and Rust. All C entry points in `harper_c` are wrapped with `std::panic::catch_unwind`. If an unexpected error or panic occurs, the function catches it, logs the details to thread-local storage accessible via `harper_last_error()`, and returns a negative error code.
4. **Thread Safety (`Send`)**:
   The checker handle (`Harper_Handle`) encapsulates an immutable curated dictionary and thread-safe linting pipeline. Handles can be dispatched across background worker threads, allowing non-blocking grammar checks in interactive UI applications.

---

## Repository Layout

```
harper-odin/
├── Cargo.toml                      # Rust crate definition (staticlib, cdylib, rlib)
├── Cargo.lock                      # Pinned Rust dependencies
├── LICENSE                         # MIT License
├── README.md                       # Repository documentation & guide
├── include/
│   └── harper_c.h                  # Canonical C-ABI header file
├── src/
│   └── lib.rs                      # extern "C" FFI implementation, panic catching, byte mapping
├── odin/
│   ├── c/
│   │   └── harper_c.odin           # Low-level Odin foreign C bindings
│   └── harper/
│       └── harper.odin             # Idiomatic high-level Odin API wrapper
├── tests/
│   ├── ffi.rs                      # Rust FFI integration tests
│   └── odin/
│       └── lint_test.odin          # Standalone Odin test suite (leak checks, UTF-8 validation)
└── .github/
    └── workflows/
        ├── test.yml                # CI test automation (cargo test + odin test)
        └── release.yml             # Automated multi-platform release packager
```

---

## C-ABI API Reference

The canonical C interface is defined in [`include/harper_c.h`](include/harper_c.h).

### Types & Enums

#### Constants & Versioning
```c
#define HARPER_C_ABI_VERSION 1

const char *harper_version(void);
uint32_t harper_abi_version(void);
```

#### `enum Harper_Dialect`
Specifies language dialects for dictionary lookups and regional spelling rules:
```c
enum Harper_Dialect {
    HARPER_DIALECT_AMERICAN   = 0,
    HARPER_DIALECT_AUSTRALIAN = 1,
    HARPER_DIALECT_BRITISH    = 2,
    HARPER_DIALECT_CANADIAN   = 3,
};
```

#### `enum Harper_Lint_Kind`
Classifies the type of issue identified by the engine:
```c
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
```

#### `enum Harper_Suggestion_Kind`
Specifies how the suggestion should be applied to the text:
```c
enum Harper_Suggestion_Kind {
    HARPER_SUGGESTION_REPLACE      = 0,
    HARPER_SUGGESTION_INSERT_AFTER = 1,
    HARPER_SUGGESTION_REMOVE       = 2,
};
```

#### `struct Harper_C_Lint`
Describes a single lint annotation. Offsets are UTF-8 byte indices:
```c
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
```

#### `struct Harper_Lint_Result`
Container for lints returned by `harper_lint`:
```c
typedef struct Harper_Lint_Result {
    Harper_C_Lint *lints;
    size_t count;
} Harper_Lint_Result;
```

#### `Harper_Handle`
Opaque handle representing an initialized checker instance:
```c
typedef struct Harper_State *Harper_Handle;
```

---

### Functions

#### `harper_new`
```c
Harper_Handle harper_new(uint32_t dialect);
```
Initializes a new Harper checker instance configured with an embedded curated dictionary for the given dialect (`Harper_Dialect`). Returns `NULL` if initialization fails.

#### `harper_destroy`
```c
void harper_destroy(Harper_Handle handle);
```
Frees the checker instance and releases internal dictionary and rule resources. Safe to pass `NULL`.

#### `harper_lint`
```c
int32_t harper_lint(Harper_Handle handle, const uint8_t *text, size_t len,
                    Harper_Lint_Result *out);
```
Performs grammar, style, and spelling checks on `text` of length `len` (in UTF-8 bytes). Populates `out` with results.
- Returns `0` on success.
- Returns negative error code on failure (`-1` = null handle, `-2` = null out pointer, `-3` = null text with non-zero length, `-4` = invalid UTF-8, `-5` = panic/internal error).

#### `harper_result_free`
```c
void harper_result_free(Harper_Lint_Result result);
```
Reclaims the whole result block allocated during `harper_lint`, including all lint structs and associated message/suggestion strings. Safe to call on empty results.

#### `harper_last_error`
```c
const char *harper_last_error(void);
```
Returns a thread-local, NUL-terminated error string describing the last error that occurred on the current thread, or an empty string `""` if no error occurred.

---

### C Usage Example

```c
#include <stdio.h>
#include <string.h>
#include "harper_c.h"

int main(void) {
    printf("Harper Core Version: %s (ABI v%u)\n", harper_version(), harper_abi_version());

    Harper_Handle handle = harper_new(HARPER_DIALECT_AMERICAN);
    if (!handle) {
        fprintf(stderr, "Initialization failed: %s\n", harper_last_error());
        return 1;
    }

    const char *text = "Café is open, but thiss is bad and they is wrong.";
    Harper_Lint_Result result = {0};

    int32_t rc = harper_lint(handle, (const uint8_t *)text, strlen(text), &result);
    if (rc != 0) {
        fprintf(stderr, "Lint error (%d): %s\n", rc, harper_last_error());
        harper_destroy(handle);
        return 1;
    }

    printf("Found %zu lint(s):\n", result.count);
    for (size_t i = 0; i < result.count; ++i) {
        Harper_C_Lint lint = result.lints[i];
        printf("  [%u..%u] kind=%u msg=\"%s\" suggestion=\"%s\"\n",
               lint.start, lint.end, lint.kind, lint.message,
               lint.suggestion ? lint.suggestion : "(none)");
    }

    harper_result_free(result);
    harper_destroy(handle);
    return 0;
}
```

---

## Odin Package Usage

`harper-odin` provides two layers of Odin bindings:
1. `odin/harper`: Idiomatic, high-level wrapper using slices, Odin strings, and custom allocator support.
2. `odin/c`: Direct foreign C-ABI bindings.

### High-Level API (`package harper`)

The high-level package provides automatic string cloning, error returns, and safe annotation slicing.

```odin
package main

import "core:fmt"
import "odin/harper"

main :: proc() {
    fmt.printf("Harper Core: %s (ABI v%d)\n", harper.version(), harper.abi_version())

    // 1. Initialize checker instance
    checker, err := harper.harper_new_checked(.American)
    if err != "" {
        fmt.eprintf("Failed to initialize checker: %s\n", err)
        return
    }
    defer harper.destroy(checker)

    // 2. Lint UTF-8 text
    text := "Café is open, but thiss sentence has an error and they is wrong."
    annotations, lint_err := harper.lint_text(checker, text)
    if lint_err != "" {
        fmt.eprintf("Linting failed: %s\n", lint_err)
        return
    }
    defer harper.destroy_annotations(annotations)

    // 3. Inspect results
    fmt.printf("Identified %d lint(s):\n", len(annotations))
    for ann in annotations {
        // Offsets are UTF-8 byte indices - safe to slice directly!
        matched_str := text[ann.start:ann.end]
        fmt.printf("  [%d..%d] '%s' -> Suggestion: '%s' (%v)\n",
            ann.start, ann.end, matched_str, ann.suggestion, ann.kind)
        fmt.printf("    Message: %s\n", ann.message)
    }
}
```

#### High-Level Structs & Procedures
- `Annotation`:
  - `start: u32`: Byte offset start in checked string.
  - `end: u32`: Byte offset end (exclusive).
  - `kind: Lint_Kind`: Enum categorization.
  - `priority: u8`: Priority level (lower is higher priority).
  - `suggestion_kind: Suggestion_Kind`: Replace, Insert_After, or Remove.
  - `message: string`: Explanatory message.
  - `suggestion: string`: Recommended replacement string.
- `harper_new_checked(dialect := .American) -> (Handle, string)`: Initializes checker, returning handle and empty string, or `(nil, error_message)`.
- `lint_text(handle: Handle, text: string, allocator := context.allocator) -> ([]Annotation, string)`: Runs lint analysis and clones strings using `allocator`.
- `destroy_annotations(annotations: []Annotation, allocator := context.allocator)`: Frees the annotation slice and strings.
- `destroy(handle: Handle)`: Reclaims the checker instance.

---

### Low-Level API (`package harper_c`)

For low-overhead, direct C-ABI foreign calls without intermediate allocations:

```odin
package main

import "core:c"
import "core:fmt"
import "odin/c"

main :: proc() {
    handle := harper_c.harper_new(.American)
    if handle == nil {
        fmt.eprintf("Error: %s\n", harper_c.harper_last_error())
        return
    }
    defer harper_c.harper_destroy(handle)

    text := "Thiss is bad."
    result: harper_c.Lint_Result

    rc := harper_c.harper_lint(handle, raw_data(text), c.size_t(len(text)), &result)
    if rc != 0 {
        fmt.eprintf("Lint error: %s\n", harper_c.harper_last_error())
        return
    }
    defer harper_c.harper_result_free(result)

    lints := result.lints[:int(result.count)]
    for lint in lints {
        fmt.printf("Lint [%d..%d]: %s -> %s\n",
            lint.start, lint.end, lint.message, lint.suggestion)
    }
}
```

---

### Configuring the Library Path

The Odin bindings link against `libharper_c.a`. By default, `odin/c/harper_c.odin` looks for the compiled static library at `../../target/release/libharper_c.a`.

You can override the static library path at compile time using the `-define:HARPER_LIB_PATH` compiler flag:

```bash
odin build . -define:HARPER_LIB_PATH=/path/to/libharper_c.a
```

---

## Local Development & Testing

### Prerequisites

- **Rust toolchain** (stable 1.75+): Needed only when developing `harper-odin` itself or compiling `libharper_c` from source.
- **Odin compiler** (dev-2024-05 or newer).

### Building from Source

To compile release builds of the static and dynamic libraries:

```bash
cargo build --release
```

This generates:
- `target/release/libharper_c.a` (Static library)
- `target/release/libharper_c.dylib` (macOS shared library) or `libharper_c.so` (Linux shared library)

### Running the Test Suites

#### 1. Rust Integration Tests
Executes C-ABI boundary tests, multibyte UTF-8 span checks, panic safety, dialect configurations, and multithreaded concurrency tests:

```bash
cargo test --verbose
```

#### 2. Odin Test Suite
The Odin test suite (`tests/odin/lint_test.odin`) verifies C-ABI linkage, high-level API correctness, multibyte UTF-8 boundary slicing (with accented characters, emojis, and CJK text), and runs through Odin's `core:mem` tracking allocator to ensure zero memory leaks:

```bash
# Ensure static library is built first
cargo build --release

# Run Odin tests with a single thread for clean allocator tracking
odin test tests/odin/ -define:ODIN_TEST_THREADS=1
```

---

## Precompiled Binary Releases & Integration

### Release Artifacts

Every GitHub release publishes platform-specific tarballs named:
`harper-odin-<TARGET>-<VERSION>.tar.gz`

Supported targets:
- `aarch64-apple-darwin` (Apple Silicon macOS)
- `x86_64-apple-darwin` (Intel macOS)
- `x86_64-unknown-linux-gnu` (Linux x86_64)

Each tarball contains:
```
harper-odin-<target>-<version>/
├── lib/
│   ├── libharper_c.a
│   └── libharper_c.dylib (or .so)
├── include/
│   └── harper_c.h
├── odin/
│   ├── c/
│   │   └── harper_c.odin
│   └── harper/
│       └── harper.odin
├── LICENSE
├── README.md
└── checksums.txt
```

Each release also publishes an accompanying SHA-256 checksum file (`harper-odin-<TARGET>-<VERSION>.tar.gz.sha256`).

### Integrating into Downstream Projects

To use `harper-odin` in a downstream project without a Rust toolchain:

1. **Download Precompiled Archive**:
   Download the appropriate release tarball for your platform from GitHub Releases and extract it into your project's vendor directory (e.g., `vendor/harper/`):

   ```bash
   mkdir -p vendor/harper
   tar -xzf harper-odin-aarch64-apple-darwin-v0.1.0.tar.gz -C vendor/harper --strip-components=1
   ```

2. **Add to Odin Compilation**:
   Import `harper` or `harper_c` in your Odin codebase:
   ```odin
   import harper "vendor/harper/odin/harper"
   ```

3. **Specify the Library Location**:
   Pass the library path flag during build or test:
   ```bash
   odin build src/ -define:HARPER_LIB_PATH=vendor/harper/lib/libharper_c.a
   ```

---

## License

This project is licensed under the **MIT License**. See the [LICENSE](LICENSE) file for the full license text.

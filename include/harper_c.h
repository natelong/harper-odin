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
